"""Integration regression: absent/unsupported hardware and optional NVML ABI failures.
Requires Linux, cargo build and a C compiler. All fixtures live in a temporary tree.
"""
import fcntl, json, os, pathlib, pty, re, select, struct, subprocess, tempfile, termios, time
binary=str(pathlib.Path('target/debug/rtop').resolve())
def drain(fd, duration):
    data=b''; end=time.monotonic()+duration
    while time.monotonic()<end:
        if select.select([fd],[],[],max(0,end-time.monotonic()))[0]: data+=os.read(fd,65536)
    return data
def tui(args,env,expected=None):
    master,slave=pty.openpty(); fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',40,120,0,0)); before=termios.tcgetattr(slave)
    p=subprocess.Popen([binary,*args],stdin=slave,stdout=slave,stderr=slave,env=env)
    try:
        out=drain(master,0.5)
        text=b' '.join(re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]',b' ',out).split())
        if expected: assert expected in text,(expected,text[-3000:])
        start=time.monotonic();os.write(master,b'q');assert p.wait(timeout=2)==0;elapsed=time.monotonic()-start
        tail=drain(master,0.1); assert b'\x1b[?1049l' in tail; assert termios.tcgetattr(slave)==before
        return elapsed
    finally:
        if p.poll() is None:p.kill();p.wait()
        os.close(master);os.close(slave)
with tempfile.TemporaryDirectory(prefix='rtop-hardware-safety-') as tmp:
    root=pathlib.Path(tmp);sysroot=root/'sys';sysroot.mkdir();lib=root/'lib';lib.mkdir()
    env={**os.environ,'TERM':'xterm-256color','XDG_CONFIG_HOME':str(root/'config')}
    env.pop('NO_COLOR',None)
    args=['--hardware-sysfs',str(sysroot)]
    def doctor(extra=(),mode=None):
        child={**env,'LD_LIBRARY_PATH':str(lib)}
        if mode:child['RTOP_FAKE_NVML']=mode
        result=subprocess.run([binary,*args,*extra,'doctor'],env=child,capture_output=True,text=True,timeout=3)
        assert result.returncode==0,result.stderr;assert '\x1b' not in result.stdout
        return result.stdout
    assert 'No GPU devices' in doctor(['--disable-nvml'])
    assert 'No temperature sensors' in doctor(['--disable-nvml'])
    tui([*args,'--disable-nvml','--section','2'],env,b'N/D: no supported GPU')
    tui([*args,'--disable-nvml','--section','6'],env,b'N/D: no sensors')
    device=sysroot/'class/drm/card0/device';device.mkdir(parents=True);(device/'vendor').write_text('0x102b')
    assert 'unsupported DRM' in doctor(['--disable-nvml'])
    tui([*args,'--disable-nvml','--section','2'],env,b'N/D: driver/backend unsupported')
    subprocess.run(['cc','-shared','-fPIC','tests/fixtures/nvml.c','-o',str(lib/'libnvidia-ml.so.1')],check=True)
    good=doctor();assert 'Ok(38.0)' in good and 'Ok(61.0)' in good and 'Ok(118.5)' in good and 'GPU-fixture-uuid' in good
    for mode,expected in [('zero','No GPU devices'),('init-error','initialization failed'),('count-error','Device Not Found'),('permission','Insufficient Permissions'),('null','null device'),('unsupported','Not Supported'),('lost','GPU Lost'),('bad-value','outside 0..100')]:
        # Remove the unrelated unsupported DRM fixture for the zero-device assertion.
        if mode=='zero':(device/'vendor').unlink();os.rmdir(device);os.rmdir(device.parent)
        output=doctor(mode=mode);assert expected in output,(mode,output)
        tui([*args,'--section','2'],{**env,'LD_LIBRARY_PATH':str(lib),'RTOP_FAKE_NVML':mode})
    subprocess.run(['cc','-shared','-fPIC','-DREQUIRED_ONLY','tests/fixtures/nvml.c','-o',str(lib/'libnvidia-ml.so.1')],check=True)
    assert 'query symbol unavailable' not in doctor() # missing names fall back to NVIDIA index
    assert 'utilization unavailable' in doctor()
    subprocess.run(['cc','-shared','-fPIC','tests/fixtures/nvml.c','-o',str(lib/'libnvidia-ml.so.1')],check=True)
    elapsed=tui([*args,'--section','2'],{**env,'LD_LIBRARY_PATH':str(lib),'RTOP_FAKE_NVML':'slow'},b'Collecting hardware')
    print(json.dumps({'status':'PASS','cases':'empty sysfs, unsupported DRM, NVML zero/init/count/permission/null/unsupported/lost/invalid/missing optional symbols, slow init','slow_backend_exit_s':elapsed,'terminal_restored':True},indent=2))
