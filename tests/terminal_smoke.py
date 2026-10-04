"""Integration smoke test: cargo build, then python3 tests/terminal_smoke.py."""
import fcntl
import os
import pty
import re
import select
import signal
import struct
import subprocess
import termios
import time


def drain(fd, duration):
    data = b""
    until = time.monotonic() + duration
    while time.monotonic() < until:
        ready, _, _ = select.select([fd], [], [], max(0, until-time.monotonic()))
        if ready:
            data += os.read(fd, 65536)
    return data


for demo, exit_key in ((False,b"q"),(False,b"\x03"),(True,b"q")):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    before = termios.tcgetattr(slave)
    process = subprocess.Popen(["target/debug/rtop", "--interval", "100"] + (["--demo"] if demo else []),
                               stdin=slave, stdout=slave, stderr=slave,
                               env={**os.environ, "TERM": "xterm-256color"})
    try:
        data = drain(master, 0.4)
        normalized=b" ".join(re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]",b" ",data).split())
        assert (b"SIMULATED DATA" if demo else b"LIVE / CPU RAM GPU DISK NET THERMALS") in normalized
        os.write(master, b" ")
        data = drain(master, 0.2)
        assert b"PAUSED" in data
        assert drain(master, 0.3) == b"", "pause must not redraw periodically"
        for rows, cols in ((24,80), (50,160), (10,40), (40,120)):
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
            os.kill(process.pid, signal.SIGWINCH)
            drain(master, 0.1)
        os.write(master, b"2\r?\x1btac")
        drain(master, 0.2)
        os.write(master, exit_key)
        assert process.wait(timeout=3) == 0
        data = drain(master, 0.1)
        assert b"\x1b[?1049l" in data, "alternate screen must be restored"
        assert termios.tcgetattr(slave) == before, "terminal settings must be restored"
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)
print("PASS: rendering, pause, resize, keyboard, q/Ctrl-C and terminal restoration")
