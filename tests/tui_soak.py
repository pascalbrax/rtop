"""Measure the real release TUI on a drained PTY. Default: one hour + 30s warmup.
Includes three five-minute windows with 30s warmup each, without restarting.
CPU counts all process threads; PTY drain/monitor overhead belongs to Python.
"""
import argparse
import csv
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pty
import resource
import select
import struct
import subprocess
import termios
import time

parser=argparse.ArgumentParser()
parser.add_argument('--seconds',type=int,default=3600)
parser.add_argument('--warmup',type=int,default=30)
parser.add_argument('--output',default='docs/benchmarks/m3-tui-soak')
args=parser.parse_args()
assert args.seconds > 0 and args.warmup >= 0
master,slave=pty.openpty()
fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',40,120,0,0))
before=termios.tcgetattr(slave)
binary=Path('target/release/rtop')
process=subprocess.Popen([str(binary),'--interval','1000'],stdin=slave,stdout=slave,stderr=slave,env={**{k:v for k,v in os.environ.items() if k!='NO_COLOR'},'COLORTERM':'truecolor','TERM':'xterm-256color','XDG_CONFIG_HOME':'/tmp/rtop-soak-no-config'})
start=time.monotonic();hz=os.sysconf('SC_CLK_TCK')
last=b'';total_bytes=0;records=[];boundaries={};checkpoints=[args.warmup]
for run in range(3):
 a=args.warmup+run*330
 if a+300 <= args.warmup+args.seconds:checkpoints.extend([a,a+300])
checkpoints=sorted(set(checkpoints+[args.warmup+args.seconds]))
next_record=args.warmup
Path(args.output).parent.mkdir(parents=True,exist_ok=True)
log=open(args.output+'.csv','w',newline='');writer=csv.writer(log)
writer.writerow(['wall_s','cpu_s','rss_kib','threads','voluntary_switches','involuntary_switches','output_bytes'])
def metrics():
 text=Path(f'/proc/{process.pid}/stat').read_text();fields=text[text.rfind(')')+2:].split()
 cpu=(int(fields[11])+int(fields[12]))/hz
 status=dict(line.split(':',1) for line in Path(f'/proc/{process.pid}/status').read_text().splitlines() if ':' in line)
 return {'wall':time.monotonic()-start,'cpu':cpu,'rss':int(status['VmRSS'].split()[0]),'threads':int(status['Threads']),
         'voluntary':int(status['voluntary_ctxt_switches']),'involuntary':int(status['nonvoluntary_ctxt_switches'])}
try:
 end=start+args.warmup+args.seconds
 while time.monotonic()<end:
  if process.poll() is not None:raise AssertionError(f'TUI exited early: {process.returncode}; {last[-1000:]!r}')
  elapsed=time.monotonic()-start
  upcoming=min([t for t in checkpoints if t>elapsed]+[next_record if next_record>elapsed else elapsed+0.05,args.warmup+args.seconds])
  ready,_,_=select.select([master],[],[],min(0.25,max(0,upcoming-elapsed)))
  if ready:
   data=os.read(master,65536);total_bytes+=len(data);last=(last+data)[-65536:]
  elapsed=time.monotonic()-start
  for t in checkpoints:
   if t not in boundaries and elapsed>=t:boundaries[t]=metrics()
  if elapsed>=next_record:
   row=metrics();records.append(row);writer.writerow([round(row['wall'],6),row['cpu'],row['rss'],row['threads'],row['voluntary'],row['involuntary'],total_bytes]);log.flush()
   print(f"elapsed={row['wall']:.0f}s CPU={row['cpu']:.2f}s RSS={row['rss']}KiB threads={row['threads']}",flush=True)
   next_record+=60
 boundaries[args.warmup+args.seconds]=metrics()
 os.write(master,b'q');assert process.wait(timeout=5)==0
 # Capture restoration bytes after exit.
 ready,_,_=select.select([master],[],[],1)
 if ready:last=(last+os.read(master,65536))[-65536:]
 assert b'\x1b[?1049l' in last
 assert termios.tcgetattr(slave)==before
 first=boundaries[args.warmup];final=boundaries[args.warmup+args.seconds]
 cpu=(final['cpu']-first['cpu'])/(final['wall']-first['wall'])*100
 windows=[]
 for run in range(3):
  a=args.warmup+run*330;b=a+300
  if a in boundaries and b in boundaries:
   one,two=boundaries[a],boundaries[b]
   windows.append({'run':run+1,'wall_s':two['wall']-one['wall'],'cpu_percent_one_core':(two['cpu']-one['cpu'])/(two['wall']-one['wall'])*100,'rss_start_kib':one['rss'],'rss_end_kib':two['rss']})
 tail=[r['rss'] for r in records if r['wall']>=args.warmup+max(0,args.seconds-900)]
 report={'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'measured_wall_s':final['wall']-first['wall'],'cpu_percent_one_core':cpu,'rss_start_kib':first['rss'],'rss_end_kib':final['rss'],'rss_tail_range_kib':max(tail)-min(tail) if tail else 0,'threads_max':max(r['threads'] for r in records),'output_bytes':total_bytes,'benchmark_windows':windows,'restored_terminal':True,'clock_ticks_per_second':hz,'net_namespace':os.readlink('/proc/self/ns/net'),'terminal':'drained PTY, 120x40, truecolor, 1 Hz; no graphical terminal emulator'}
 Path(args.output+'.json').write_text(json.dumps(report,indent=2)+'\n')
 Path(args.output+'-tail.ansi').write_bytes(last)
 print(json.dumps(report,indent=2),flush=True)
 assert cpu<=0.5,f'CPU budget exceeded: {cpu:.3f}%'
 assert all(w['cpu_percent_one_core']<=0.5 for w in windows),'window CPU budget exceeded'
 assert report['rss_tail_range_kib']<=256,'RSS keeps growing in final measurement window'
finally:
 log.close()
 if process.poll() is None:process.kill();process.wait()
 os.close(master);os.close(slave)
