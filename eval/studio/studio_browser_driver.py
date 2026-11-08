"""Original stdlib loopback WebDriver BiDi test driver; never a game dependency.
Wire commands follow https://w3c.github.io/webdriver-bidi/.
"""
import base64,hashlib,json,pathlib,socket,struct,os,time
class Bidi:
 def __init__(self,port=9223):
  self.s=socket.create_connection(('127.0.0.1',port),timeout=5);self.s.settimeout(10);key=base64.b64encode(os.urandom(16)).decode();self.s.sendall(f'GET /session HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n'.encode());h=b''
  while not h.endswith(b'\r\n\r\n'):h+=self.s.recv(1)
  assert b'101' in h,h;self.n=0;self.events=[]
 def read(self,n):
  b=b''
  while len(b)<n:
   more=self.s.recv(n-len(b));assert more;b+=more
  return b
 def send(self,data,opcode=1):
  p=data if isinstance(data,bytes) else json.dumps(data).encode();n=len(p);header=bytes([128|opcode,128|(n if n<126 else 126 if n<65536 else 127)])
  if n>=126:header+=struct.pack('!H' if n<65536 else '!Q',n)
  mask=os.urandom(4);self.s.sendall(header+mask+bytes(b^mask[i%4] for i,b in enumerate(p)))
 def call(self,method,params):
  self.n+=1;n=self.n;self.send(dict(id=n,method=method,params=params));chunks=b''
  while True:
   a,b=self.read(2);length=b&127
   if length==126:length=struct.unpack('!H',self.read(2))[0]
   elif length==127:length=struct.unpack('!Q',self.read(8))[0]
   assert length<=8*1024*1024
   mask=self.read(4) if b&128 else None;p=self.read(length)
   if mask:p=bytes(v^mask[i%4] for i,v in enumerate(p))
   if a&15==9:self.send(p,10);continue
   assert a&15!=8,p;chunks+=p
   if not a&128:continue
   v=json.loads(chunks);chunks=b''
   if v.get('id')==n:
    assert v.get('type')!='error',v;return v['result']
   self.events.append(v)
   if len(self.events)>100:self.events.pop(0)
 def evaluate(self,expr):
  r=self.call('script.evaluate',dict(expression=expr,target=dict(context=self.context),awaitPromise=True));assert r['type']=='success',r;return r['result'].get('value')
