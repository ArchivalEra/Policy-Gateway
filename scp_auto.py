#!/usr/bin/env python3
import sys, os, pty, time, select

host = sys.argv[1]
password = sys.argv[2]
local = sys.argv[3]
remote = sys.argv[4]

pid, fd = pty.fork()
if pid == 0:
    os.execvp("scp", ["scp", "-F", "/dev/null", "-P", "22",
                      "-o", "StrictHostKeyChecking=no",
                      "-o", "UserKnownHostsFile=/dev/null",
                      local, f"{host}:{remote}"])
else:
    output = b""
    time.sleep(2)
    while True:
        try:
            r, w, e = select.select([fd], [], [], 0.5)
            if r:
                data = os.read(fd, 4096)
                if not data:
                    break
                output += data
                if b"assword" in data or b"password" in data.lower():
                    os.write(fd, (password + "\n").encode())
            else:
                pid2, status = os.waitpid(pid, os.WNOHANG)
                if pid2 != 0:
                    break
        except OSError:
            break
    os.waitpid(pid, 0)
    print(output.decode(errors='replace'))
