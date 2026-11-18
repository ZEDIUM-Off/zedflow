"""Isolated real-PTY API-key prompt/interrupt probe. No provider network call."""
import errno
import json
import os
import pty
import select
import subprocess
import sys
import termios
import time

node, entry, cwd, global_directory, mode = sys.argv[1:]
master, slave = pty.openpty()
before = termios.tcgetattr(slave)
arguments = ['run', 'counter' if mode == 'tui' else 'agent', '--tui'] if mode.startswith('tui') else ['auth', 'login', '--api-key']
process = subprocess.Popen(
    [node, entry, '--cwd', cwd, '--global-dir', global_directory, *arguments],
    stdin=slave, stdout=slave, stderr=slave, close_fds=True,
)
output = bytearray()
sent = False
prompt_safe = False
phase = 0
deadline = time.monotonic() + 12
try:
    while time.monotonic() < deadline:
        ready, _, _ = select.select([master], [], [], 0.05)
        if ready:
            try:
                chunk = os.read(master, 65536)
            except OSError as error:
                if error.errno == errno.EIO:
                    break
                raise
            output.extend(chunk)
            if mode == 'tui':
                if phase == 0 and b'Type a JSON event' in output:
                    os.write(master, b'{"type":"INCREMENT"}\r')
                    phase = 1
                    sent = True
                elif phase == 1 and b'"count": 1' in output:
                    os.write(master, b'/reload\r')
                    phase = 2
                elif phase == 2 and b'Reload applied.' in output:
                    os.write(master, b'/quit\r')
                    phase = 3
            elif mode == 'tui-login':
                if phase == 0 and b'Type a prompt' in output:
                    os.write(master, b'/login api-key\r')
                    phase = 1
                elif phase == 1 and b'Enter OpenAI API key' in output:
                    prompt_safe = not bool(termios.tcgetattr(slave)[3] & termios.ECHO)
                    os.write(master, b'synthetic-pty-key\r')
                    phase = 2
                    sent = True
                elif phase == 2 and b'Signed in to OpenAI' in output:
                    os.write(master, b'/quit\r')
                    phase = 3
            elif not sent and b'api key' in output.lower():
                prompt_safe = not bool(termios.tcgetattr(slave)[3] & termios.ECHO)
                os.write(master, b'\x03' if mode == 'cancel' else b'synthetic-pty-key\r')
                sent = True
        if process.poll() is not None:
            break
    if process.poll() is None:
        raise RuntimeError('CLI did not exit after its auth prompt')
    after = termios.tcgetattr(slave)
    print(json.dumps({'code': process.returncode, 'phase': phase, 'sent': sent, 'promptSafe': prompt_safe, 'restored': before == after, 'output': output.decode('utf8', 'replace')}))
finally:
    if process.poll() is None:
        process.kill()
    process.wait()
    os.close(master)
    os.close(slave)
