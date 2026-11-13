sub show(label as string, aa as object)
 result=[]
 for each key in aa
  result.push(key)
 end for
 print label;"=";result.join(",")
end sub
sub Main()
 print "@@PROBE_BEGIN"
 keys=["a","b","c","d","e","f","g","h","i","j","k","l","m","n","o","p","q","r","s","t","u","v","w","x","y","z"]
 for n=1 to 26
  aa={}
  for i=0 to n-1
   aa[keys[i]]=i
  end for
  show("n"+n.toStr(),aa)
 end for
 for n=1 to 26
  aa={}
  for i=n-1 to 0 step -1
   aa[keys[i]]=i
  end for
  show("r"+n.toStr(),aa)
 end for
 aa={b:2,a:1,zed:"z"}
 aa.c=3
 aa["D"]=4
 show("original",aa)
 aa.delete("a")
 show("deleted",aa)
 aa["a"]=10
 show("reinserted",aa)
 aa={}
 for each key in keys
  aa[key]=1
 end for
 for i=4 to 25
  aa.delete(keys[i])
 end for
 show("retained capacity",aa)
 aa.clear()
 for i=0 to 3
  aa[keys[i]]=1
 end for
 show("cleared capacity",aa)
 names=["title","description","id","translation","width","height","field1","field2","0","1","9","10","alpha","ALPHA","beta","one-two","one_two","empty","","zebra","Zed","itemContent","longer_key_name","a.b","é","ç","á","Ω","你好"]
 aa={}
 for each key in names
  aa[key]=key
 end for
 show("named keys",aa)
 aa["TITLE"]="updated"
 show("overwrite",aa)
 print "@@PROBE_END"
 screen=CreateObject("roSGScreen")
 scene=screen.CreateScene("ProbeScene")
 screen.Show()
 while true
  sleep(1000)
 end while
end sub
