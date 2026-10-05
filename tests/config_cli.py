"""Verify TOML loading, CLI precedence, environment and errors before terminal init."""
import os
from pathlib import Path
import subprocess
import tempfile
BIN='target/release/rtop'
env={**os.environ,'NO_COLOR':'1'}
with tempfile.TemporaryDirectory(prefix='rtop-config-') as directory:
 path=Path(directory)/'config.toml'
 path.write_text('interval=150\nhistory=10\nascii=true\ntheme="light"\n')
 def run(*args):return subprocess.run([BIN,'--config',str(path),*args],env=env,text=True,capture_output=True)
 result=run('--preview','80x24')
 assert result.returncode==0 and result.stdout.isascii()
 result=run('--preview','80x24','--ascii=false','--no-color=false','--theme','dark','--svg')
 assert result.returncode==0 and '#242522' in result.stdout and '╭' in result.stdout
 for text in ('interval=0','history=0','theme="unknown"','unknown_option=true','interval="fast"','filesystem_interval=0','process_interval=0'):
  path.write_text(text);result=run()
  assert result.returncode!=0
  assert '\x1b[?1049h' not in result.stdout
 path.write_text('interval=0')
 result=run('--preview','80x24','--interval','1000')
 assert result.returncode==0,'explicit CLI interval must override file value before validation'
 path.unlink();result=run('--preview','80x24');assert result.returncode!=0
print('PASS: TOML, CLI precedence, NO_COLOR override, strict keys/types/bounds and pre-terminal errors')
