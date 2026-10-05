"""Real PTY and inotify: demand-only scans, filtering, pause, resize and blocked reads."""
import ctypes, fcntl, json, os, pty, select, signal, struct, subprocess, tempfile, termios, time
from pathlib import Path

BIN = 'target/release/rtop'
def stat(pid, ticks=0):
    fields=['0']*22
    fields[0]='S'; fields[11]=str(ticks); fields[19]=str(pid*100); fields[21]='1024'
    return f'{pid} (worker-{pid}) '+ ' '.join(fields)
def drain(fd, seconds):
    end=time.monotonic()+seconds; data=b''
    while time.monotonic()<end:
        ready,_,_=select.select([fd],[],[],max(0,end-time.monotonic()))
        if ready:
            try: data+=os.read(fd,65536)
            except OSError: break
    return data
def start(root, rows=24, cols=80):
    master,slave=pty.openpty()
    fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',rows,cols,0,0))
    before=termios.tcgetattr(slave)
    p=subprocess.Popen([BIN,'--process-proc',str(root),'--process-interval','1000','--no-color','false','--ascii','false'],stdin=slave,stdout=slave,stderr=slave,
                       env={**os.environ,'TERM':'xterm-256color','XDG_CONFIG_HOME':str(root/'no-config')})
    return p,master,slave,before
def close(p,master,slave,before):
    os.write(master,b'q');assert p.wait(timeout=2)==0
    assert termios.tcgetattr(slave)==before
    assert b'\x1b[?1049l' in drain(master,.1)
    os.close(master);os.close(slave)

with tempfile.TemporaryDirectory(prefix='rtop-process-safety-') as temp:
    root=Path(temp)
    for pid in range(1,65):
        (root/str(pid)).mkdir();(root/str(pid)/'stat').write_text(stat(pid))
    libc=ctypes.CDLL(None,use_errno=True)
    libc.inotify_init1.argtypes=[ctypes.c_int];libc.inotify_init1.restype=ctypes.c_int
    libc.inotify_add_watch.argtypes=[ctypes.c_int,ctypes.c_char_p,ctypes.c_uint32];libc.inotify_add_watch.restype=ctypes.c_int
    notify=libc.inotify_init1(os.O_NONBLOCK|os.O_CLOEXEC);assert notify>=0
    for path in root.glob('*/stat'):assert libc.inotify_add_watch(notify,os.fsencode(path),0x20)>=0 # IN_OPEN
    def opens():
        count=0
        while True:
            try:data=os.read(notify,65536)
            except BlockingIOError:return count
            offset=0
            while offset<len(data):
                _,mask,_,length=struct.unpack_from('iIII',data,offset);offset+=16+length
                if mask&0x20:count+=1
    p,master,slave,before=start(root)
    try:
        drain(master,.5);assert opens()==0,'hidden CPU view must never open process stats'
        os.write(master,b'7');data=drain(master,.6);assert opens()>0
        assert b'worker-' in data and b'/ DEMO' not in data
        os.write(master,b'/worker-64\r');drain(master,.3)
        os.write(master,b't');data=drain(master,.2)
        assert b'worker-64' in data
        os.write(master,b'/q');drain(master,.1);assert p.poll() is None,'q is filter text while editing'
        os.write(master,b'\x1b');drain(master,.1)
        os.write(master,b'sr\x1b[B\x1b[6~\x1b[H\x1b[F');drain(master,.2);assert p.poll() is None
        os.write(master,b'1');drain(master,.2);opens();drain(master,1.2);assert opens()==0,'hidden view keeps scanning'
        fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',40,120,0,0));os.kill(p.pid,signal.SIGWINCH)
        data=drain(master,1.3);count=opens();assert count>0,'overview must enable process sampling after resize'
        os.write(master,b' ');drain(master,.2);opens();assert drain(master,1.2)==b'';assert opens()==0,'paused scans'
        os.write(master,b' ');drain(master,.5);assert opens()>0
        os.write(master,b'?');drain(master,.2);opens();drain(master,1.2);assert opens()==0,'help suspends scans'
        os.write(master,b'\x1b');drain(master,.5);assert opens()>0
        close(p,master,slave,before)
    finally:
        if p.poll() is None:p.kill();p.wait();os.close(master);os.close(slave)
        os.close(notify)

with tempfile.TemporaryDirectory(prefix='rtop-process-blocked-') as temp:
    root=Path(temp);(root/'1').mkdir();os.mkfifo(root/'1/stat')
    p,master,slave,before=start(root)
    try:
        drain(master,.3);os.write(master,b'7');drain(master,.3)
        begin=time.monotonic();close(p,master,slave,before);exit_ms=(time.monotonic()-begin)*1000
    finally:
        if p.poll() is None:p.kill();p.wait();os.close(master);os.close(slave)
result={'status':'PASS','fixture_processes':64,'checks':['inotify: zero reads while hidden','overview enables scans','filter and navigation','pause and help stop reads','q during blocked FIFO read','terminal restoration'],'blocked_exit_ms_including_100ms_drain':exit_ms}
Path('docs/benchmarks/m5-process-safety.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
