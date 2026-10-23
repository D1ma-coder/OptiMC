#!/usr/bin/env python3
"""Development-only JAR protocol inventory. Never starts Minecraft or a server/GPU."""
import argparse, hashlib, json, pathlib, re, subprocess, zipfile
p=argparse.ArgumentParser();p.add_argument('--jar',default='.cache/26.2/client.jar');p.add_argument('--javap',default='/Library/Java/JavaVirtualMachines/temurin-27.jdk/Contents/Home/bin/javap');a=p.parse_args()
root=pathlib.Path(__file__).resolve().parents[1];jar=(root/a.jar).resolve();reference=json.loads((root/'data/26.2/reference.json').read_text());assert hashlib.sha1(jar.read_bytes()).hexdigest()==reference['client_sha1']
out=root/'.cache/protocol-audit';out.mkdir(parents=True,exist_ok=True)
protocol_classes={phase:f'net.minecraft.network.protocol.{phase}.{name}Protocols' for phase,name in [('handshake','Handshake'),('status','Status'),('login','Login'),('configuration','Configuration'),('game','Game')]}
rows=[]
for phase,cls in protocol_classes.items():
 text=subprocess.check_output([a.javap,'-classpath',str(jar),'-p','-c',cls],text=True);(out/f'{phase}-protocols.txt').write_text(text)
 # Registration order is wire packet ID; common/cookie packet types are interleaved.
 seen={'clientbound':set(),'serverbound':set()};counts={'clientbound':0,'serverbound':0}
 lines=text.splitlines()
 for i,line in enumerate(lines):
  m=re.search(r'// Field [\w/]+(?:PacketTypes)\.(CLIENTBOUND|SERVERBOUND)_([A-Z0-9_]+):',line)
  if not m:
   if phase=='handshake' and 'HandshakePacketTypes.CLIENT_INTENTION:' in line:
    flow='serverbound';name='client_intention'
   else:continue
  else:flow=m[1].lower();name=m[2].lower()
  if name in seen[flow]:continue
  # PacketType could appear elsewhere; require a codec registration immediately after it.
  window='\n'.join(lines[i+1:i+10]);codec=re.search(r'// Field ([\w/$]+)\.STREAM_CODEC:',window)
  if not codec and 'withBundlePacket' not in window:raise RuntimeError(f'No codec registration: {phase} {name}')
  packet_class=codec[1].replace('/','.') if codec else 'net.minecraft.network.protocol.game.ClientboundBundleDelimiterPacket'
  rows.append({'phase':'play' if phase=='game' else phase,'flow':flow,'id':counts[flow],'name':name,'packet_class':packet_class,'registration_reference':f'{phase}-protocols.txt:{i+1}'})
  counts[flow]+=1;seen[flow].add(name)
 print(phase,counts)
classes=sorted({r['packet_class'] for r in rows})
# javap only reads class files. Full payload codecs and handle(listener) targets remain available for review.
text=subprocess.check_output([a.javap,'-classpath',str(jar),'-p','-c',*classes],text=True);(out/'packet-codecs.txt').write_text(text)
listeners=['net.minecraft.client.multiplayer.ClientPacketListener','net.minecraft.client.multiplayer.ClientCommonPacketListenerImpl','net.minecraft.client.multiplayer.ClientConfigurationPacketListenerImpl','net.minecraft.client.multiplayer.ClientHandshakePacketListenerImpl','net.minecraft.client.multiplayer.ServerStatusPinger$1','net.minecraft.client.player.LocalPlayer']
text=subprocess.check_output([a.javap,'-classpath',str(jar),'-p','-c',*listeners],text=True,stderr=subprocess.STDOUT);(out/'client-handlers.txt').write_text(text)
assert 'Error: class not found:' not in text, 'Missing reference listener class'
# Extract method names and outgoing packet construction for every listener handler.
blocks=re.split(r'(?=^  (?:public|protected|private) )',text,flags=re.M)
handlers={}
for b in blocks:
 head=b.splitlines()[0] if b else ''
 m=re.search(r'\b(\w+)\((net\.minecraft\.network\.protocol\.[\w.$]+)\)',head)
 if m:
  handlers[m[2]]={'method':m[1], 'calls':sorted(set(re.findall(r'// (?:Interface)?Method ([\w/$]+\.[\w$<>]+):',b))), 'outgoing_packet_classes':sorted(set(s.replace('/','.') for s in re.findall(r'// class (net/minecraft/network/protocol/[\w/$]+Packet)',b)))}
for r in rows:
 if r['flow']=='clientbound':r['official_handler']=handlers.get(r['packet_class']) or handlers.get(r['packet_class'].split('$')[0])
# A missing handler or duplicate must fail the audit, not produce a plausible count.
assert len(rows) == 256, 'Unexpected packet registry size for pinned 26.2 JAR'
assert len({(r['phase'], r['flow'], r['id']) for r in rows}) == len(rows)
assert len({(r['phase'], r['flow'], r['name']) for r in rows}) == len(rows)
assert all(r.get('official_handler') for r in rows if r['flow'] == 'clientbound'), 'Unresolved original listener handler'
result={'minecraft':'26.2','protocol':776,'source_client_sha1':reference['client_sha1'],'method':'javap registration order, packet STREAM_CODEC/handle bytecode, client listener method bodies; no client/server execution','packets':rows}
(root/'docs/protocol-776-inventory.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
print('Total packet registrations:',len(rows),'client handlers:',sum(r.get('official_handler') is not None for r in rows))

# Audit every ID against the previously verified map, rather than trusting packet counts alone.
old=json.loads((root/'docs/protocol-776.json').read_text())
for phase in ['play','configuration']:
 for flow in ['clientbound','serverbound']:
  actual={r['name']:r['id'] for r in rows if r['phase']==phase and r['flow']==flow}
  assert actual==old[phase][flow],(phase,flow,'protocol ID mismatch')
native='//! Generated from the SHA-1 verified 26.2 JAR by tools/audit_protocol.py.\n'
native+="pub fn name(phase:&str,flow:&str,id:i32)->Option<&'static str>{match (phase,flow,id){\n"
for r in rows:native+=f'    ("{r["phase"]}","{r["flow"]}",{r["id"]})=>Some("{r["name"]}"),\n'
native+='    _=>None,\n}}\n'
(root/'src/net/protocol.rs').write_text(native)
