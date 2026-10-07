"""PTY terminal profiles, resize storms, no-color and clean pre-init errors.
These are protocol tests, not tests of real graphical terminal emulators.
"""
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time

BIN = str(Path('target/release/rtop').resolve())


def drain(fd, seconds):
    data = b''
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        if select.select([fd], [], [], max(0, end - time.monotonic()))[0]:
            data += os.read(fd, 65536)
    return data


profiles = [
    ('xterm-256color', [], False),
    ('screen-256color', ['--light'], False),
    ('linux', ['--ascii', '--no-color'], True),
    ('vt100', ['--ascii', '--no-color'], True),
    ('dumb', ['--ascii', '--no-color'], True),
]
results = []
with tempfile.TemporaryDirectory(prefix='rtop-terminal-matrix-') as directory:
    for index, (term, options, plain) in enumerate(profiles):
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 120, 0, 0))
        before = termios.tcgetattr(slave)
        env = {k: v for k, v in os.environ.items() if k not in ('NO_COLOR', 'COLORTERM')}
        env.update(TERM=term, XDG_CONFIG_HOME=directory)
        if not plain:
            env['COLORTERM'] = 'truecolor'
        process = subprocess.Popen([BIN, '--demo', *options], stdin=slave, stdout=slave,
                                   stderr=slave, env=env)
        try:
            data = drain(master, .3)
            text = b' '.join(re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b' ', data).split())
            assert process.poll() is None and b'SIMULATED DATA' in text, (term, process.poll(), data[:1000])
            if plain:
                assert data.isascii(), (term, data)
                for sgr in re.findall(rb'\x1b\[([0-9;]*)m', data):
                    codes = [int(code) for code in sgr.split(b';') if code]
                    assert not any(code in (38, 48, 58) or 30 <= code <= 37 or
                                   40 <= code <= 47 or 90 <= code <= 97 or
                                   100 <= code <= 107 for code in codes), (term, sgr)
            os.write(master, b' ')
            drain(master, .15)
            assert drain(master, .15) == b'', 'paused UI redraws periodically'
            for cycle in range(4):
                for rows, cols in [(18, 60), (24, 80), (40, 120), (50, 160), (10, 40)]:
                    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
                    os.kill(process.pid, signal.SIGWINCH)
                    drain(master, .03)
                    assert process.poll() is None
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 120, 0, 0))
            os.kill(process.pid, signal.SIGWINCH)
            drain(master, .1)
            for key in b'1234567':
                os.write(master, bytes([key]) + b'\r\x1b')
                drain(master, .04)
            os.write(master, b'?')
            assert b'KEYBOARD' in drain(master, .15)
            os.write(master, b'\x1b')
            drain(master, .1)
            os.write(master, b'\x03' if index % 2 else b'q')
            assert process.wait(timeout=3) == 0
            assert b'\x1b[?1049l' in drain(master, .1)
            assert termios.tcgetattr(slave) == before
            results.append({'term': term, 'ascii_no_color': plain, 'resize_count': 21,
                            'terminal_restored': True})
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            os.close(master)
            os.close(slave)
    master, slave = pty.openpty()
    before = termios.tcgetattr(slave)
    try:
        # TTY input + redirected stdout must fail before raw mode/alternate screen.
        result = subprocess.run([BIN], stdin=slave, capture_output=True,
                                env={**os.environ, 'XDG_CONFIG_HOME': directory}, timeout=3)
        assert result.returncode != 0 and b'interactive mode requires terminal' in result.stderr
        assert b'panicked' not in result.stderr and b'\x1b' not in result.stdout + result.stderr
        assert termios.tcgetattr(slave) == before
        result = subprocess.run([BIN, '--collect', '1'], capture_output=True, timeout=3)
        assert result.returncode == 0 and b'memory\t' in result.stdout
    finally:
        os.close(master)
        os.close(slave)
report = {'status': 'PASS', 'scope': 'drained PTY protocol profiles; no emulator/SSH claim',
          'profiles': results, 'redirected_output_error_before_terminal_init': True,
          'headless_without_tty': True}
Path('docs/benchmarks/m6-terminal-matrix.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
