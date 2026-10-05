"""Compare the real table's CPU% with an independent tick/wall-time observation."""
import json, os, subprocess, sys, tempfile, time
from pathlib import Path

busy=subprocess.Popen([sys.executable,'-c','while True: pass'])
def ticks():
    fields=Path(f'/proc/{busy.pid}/stat').read_text().rsplit(')',1)[1].split()
    return int(fields[11])+int(fields[12])
try:
    time.sleep(.1)
    initial=ticks();start=time.monotonic()
    with tempfile.TemporaryDirectory(prefix='rtop-process-live-') as directory:
        output=subprocess.check_output(['target/release/rtop','--live-preview','--preview','120x40','--section','7','--ascii','true','--no-color','true','--interval','1000'],
                                       env={**os.environ,'XDG_CONFIG_HOME':directory},text=True)
    end=time.monotonic();final=ticks()
    reference=100*(final-initial)/(os.sysconf('SC_CLK_TCK')*(end-start))
    row=next(line.strip('| ').split() for line in output.splitlines() if line.strip('| ').split() and line.strip('| ').split()[0]==str(busy.pid))
    reported=float(row[-3]);rss_mib=float(row[-2])
    assert abs(reported-reference)<=15,(reported,reference)
    assert reported>10 and rss_mib>0
    result={'status':'PASS','reference_cpu_percent':reference,'table_cpu_percent':reported,'rss_mib':rss_mib,'method':'single busy child; independent /proc tick delta and wall time bracketing a 1s live preview; tolerance 15 percentage points'}
    Path('docs/benchmarks/m5-process-live-verification.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))
finally:
    busy.terminate();busy.wait(timeout=3)
