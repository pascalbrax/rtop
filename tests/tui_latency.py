"""Input-to-completed-frame latency on a real release TUI with active collectors."""
import argparse,fcntl,json,os,pty,select,struct,subprocess,termios,time
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('--output',default='docs/benchmarks/m3-input-latency.json')
parser.add_argument('--warmup',type=float,default=125)
parser.add_argument('--process-proc')
parser.add_argument('--process-actions',action='store_true')
parser.add_argument('--section',type=int,choices=range(1,8))
args=parser.parse_args()
assert args.warmup>=0
master,slave=pty.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',40,120,0,0))
command=['target/release/rtop']
if args.process_proc:command+=['--process-proc',args.process_proc]
if args.section:command+=['--section',str(args.section)]
p=subprocess.Popen(command,stdin=slave,stdout=slave,stderr=slave,env={**{k:v for k,v in os.environ.items() if k!='NO_COLOR'},'COLORTERM':'truecolor','TERM':'xterm-256color','XDG_CONFIG_HOME':'/tmp/rtop-latency-no-config'})
def drain(seconds):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  ready,_,_=select.select([master],[],[],max(0,end-time.monotonic()))
  if ready:os.read(master,65536)
try:
 drain(args.warmup);latencies=[]
 for i in range(40):
  target=b'48;2;245;247;250' if i%2==0 else b'48;2;36;37;34'
  data=b'';start=time.monotonic()
  payload=(b'/worker-9999\rt' if i%2==0 else b'/'+b'\x7f'*11+b'\rt') if args.process_actions else b't'
  os.write(master,payload)
  while True:
   ready,_,_=select.select([master],[],[],1)
   assert ready,f'UI unresponsive, step={i}, target={target!r}, data={data[:300]!r}, tail={data[-100:]!r}'
   data+=os.read(master,65536)
   at=data.find(target)
   if at>=0 and b'\x1b[?25l' in data[at:]:break
   assert time.monotonic()-start<2
  latencies.append((time.monotonic()-start)*1000);drain(0.05)
 os.write(master,b'q');assert p.wait(timeout=3)==0
 ordered=sorted(latencies);p95=ordered[(len(ordered)-1)*95//100]
 result={'process_actions':args.process_actions,'process_proc':args.process_proc,'section':args.section,'samples':len(latencies),'p50_ms':ordered[len(ordered)//2],'p95_ms':p95,'max_ms':max(latencies),'history_warmup_s':args.warmup,'method':('filter entry/clear plus theme key to completed frame' if args.process_actions else 'theme key to target palette plus end-of-frame cursor-hide sequence')+'; 120x40 drained PTY; live collectors'}
 Path(args.output).parent.mkdir(parents=True,exist_ok=True)
 Path(args.output).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
 assert p95<=100
finally:
 if p.poll() is None:p.kill();p.wait()
 os.close(master);os.close(slave)
