"""Intel Arc backend integration: fake DRM/hwmon and runtime Sysman failures.
No Intel GPU, Level Zero installation or privileges are required; C compiler needed.
"""
import fcntl,json,os,pathlib,pty,re,select,struct,subprocess,tempfile,termios,time
binary=str(pathlib.Path('target/debug/rtop').resolve())
def drain(fd,seconds):
    end=time.monotonic()+seconds;data=b''
    while time.monotonic()<end:
        if select.select([fd],[],[],max(0,end-time.monotonic()))[0]:data+=os.read(fd,65536)
    return data
def tui(args,env,expected,wait=0.6):
    master,slave=pty.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',40,120,0,0));before=termios.tcgetattr(slave)
    p=subprocess.Popen([binary,*args,'--section','2'],stdin=slave,stdout=slave,stderr=slave,env=env)
    try:
        text=b' '.join(re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]',b' ',drain(master,wait)).split());assert expected in text,(expected,text[-2000:])
        start=time.monotonic();os.write(master,b'q');assert p.wait(timeout=2)==0;elapsed=time.monotonic()-start
        assert b'\x1b[?1049l' in drain(master,0.1);assert termios.tcgetattr(slave)==before
        return elapsed
    finally:
        if p.poll() is None:p.kill();p.wait()
        os.close(master);os.close(slave)
with tempfile.TemporaryDirectory(prefix='rtop-intel-test-') as tmp:
    root=pathlib.Path(tmp);sysroot=root/'sys';lib=root/'lib';lib.mkdir()
    dev=sysroot/'devices/pci0000:00/0000:03:00.0';dev.mkdir(parents=True)
    (dev/'vendor').write_text('0x8086');(dev/'device').write_text('0x56a0');(dev/'product_name').write_text('Fixture Intel Arc')
    (dev/'driver').symlink_to('/drivers/xe')
    card=sysroot/'class/drm/card0';card.mkdir(parents=True);(card/'device').symlink_to(dev)
    hw=sysroot/'class/hwmon/hwmon0';hw.mkdir(parents=True);(hw/'device').symlink_to(dev)
    (hw/'name').write_text('xe');(hw/'temp1_input').write_text('48000');(hw/'temp1_label').write_text('GPU edge')
    env={**os.environ,'TERM':'xterm-256color','NO_COLOR':'1','XDG_CONFIG_HOME':str(root/'config'),'LD_LIBRARY_PATH':str(lib)}
    args=['--hardware-sysfs',str(sysroot),'--disable-nvml','--hardware-interval','1000']
    def doctor(mode=None,extra=()):
        child={**env};
        if mode:child['RTOP_FAKE_INTEL']=mode
        out=subprocess.run([binary,*args,*extra,'doctor'],env=child,text=True,capture_output=True,timeout=3)
        assert out.returncode==0,out.stderr;return out.stdout
    assert 'Intel Sysman disabled' in doctor(extra=['--disable-intel'])
    assert 'GPU | xe | GPU edge | Ok(48.0)' in doctor(extra=['--disable-intel'])
    tui([*args,'--disable-intel'],env,b'N/D: Intel Sysman disabled')
    def compile(extra=()):subprocess.run(['cc','-shared','-fPIC',*extra,'tests/fixtures/intel_sysman.c','-o',str(lib/'libze_loader.so.1')],check=True)
    compile()
    closed=subprocess.Popen([binary,*args,'doctor'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    closed.stdout.close();assert closed.wait(timeout=3)==0;assert b'panicked' not in closed.stderr.read()
    out=doctor();assert 'Intel Level Zero Sysman' in out and 'Ok((2147483648, 8589934592))' in out and 'Ok(58.5)' in out
    assert 'GPU maximum' in out and 'critical None' in out # reporting maximum is not critical temperature
    assert out.count('GPU '+str(dev))==1 # DRM and Sysman identify the same device
    # Two real sample timestamps/counters are needed; preview must prime Intel rates.
    preview=subprocess.run([binary,*args,'--preview','120x40','--live-preview','--interval','100','--section','2'],env=env,text=True,capture_output=True,timeout=3)
    assert preview.returncode==0 and '25.0% GPU' in preview.stdout,preview.stdout
    # Pausing must not compute a utilization delta across the suspended period.
    master,slave=pty.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',40,120,0,0));before=termios.tcgetattr(slave)
    p=subprocess.Popen([binary,*args,'--section','2'],stdin=slave,stdout=slave,stderr=slave,env=env)
    try:
        assert b'25.0%' in drain(master,1.4)
        os.write(master,b' ');drain(master,0.2)
        assert drain(master,1.2)==b'', 'paused hardware must not redraw'
        os.write(master,b' ');assert b'collecting' in drain(master,0.4)
        assert b'25.0%' in drain(master,1.1)
        os.write(master,b'q');assert p.wait(timeout=2)==0
        assert b'\x1b[?1049l' in drain(master,0.1);assert termios.tcgetattr(slave)==before
    finally:
        if p.poll() is None:p.kill();p.wait()
        os.close(master);os.close(slave)
    for driver in ['i915','xe']:
        (dev/'driver').unlink();(dev/'driver').symlink_to('/drivers/'+driver)
        assert 'Intel Level Zero Sysman' in doctor()
    cases=[('zero','device unavailable'),('no-device','device unavailable'),('init-error','uninitialized'),('count-error','device lost'),('null','null or duplicate handle'),('growing','enumeration changed'),('excessive','excessive handle count'),('wrong-pci','device unavailable'),('pci-error','insufficient permissions'),('unsupported','unsupported feature'),('permission','insufficient permissions'),('lost','device lost'),('bad-memory','invalid VRAM'),('bad-temp','invalid temperature'),('system-memory','device-local VRAM unavailable'),('single-engine','whole-device engine counter unavailable')]
    for mode,expected in cases:
        out=doctor(mode);assert expected in out,(mode,out)
        assert 'GPU | xe | GPU edge | Ok(48.0)' in out # fallback hwmon continues on every Sysman error
        tui(args,{**env,'RTOP_FAKE_INTEL':mode},b'GPU')
    for mode,expected in [('reset','reset'),('no-progress','no progress'),('bad-activity','outside 0..100')]:
        out=subprocess.run([binary,*args,'--preview','120x40','--live-preview','--interval','100','--section','2'],env={**env,'RTOP_FAKE_INTEL':mode},text=True,capture_output=True,timeout=3)
        assert out.returncode==0 and expected in out.stdout,(mode,out.stdout)
    assert 'Ok((2147483648, 8589934592))' in doctor('tiles') # no root/tile double count
    assert 'Ok((2147483648, 8589934592))' in doctor('old-memory-size')
    compile(['-DREQUIRED_ONLY']);assert 'properties unavailable' in doctor()
    compile(['-DOLD_LOADER']);assert 'zesInit unavailable' in doctor()
    compile();elapsed=tui(args,{**env,'RTOP_FAKE_INTEL':'slow'},b'Collecting hardware')
    print(json.dumps({'status':'PASS','drivers':['i915','xe'],'cases':[c[0] for c in cases]+['reset','no-progress','bad-activity','tiles','old-memory-size','missing optional symbols','old loader','closed doctor output pipe','pause/resume re-primes counters','slow init'],'slow_backend_exit_s':elapsed,'terminal_restored':True},indent=2))
