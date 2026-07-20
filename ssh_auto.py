#!/usr/bin/env python3
import subprocess, sys, os

target = sys.argv[1]
password = sys.argv[2]
cmd = sys.argv[3:]

# Use pty to automate password prompt
import pty, time, select

def ssh_with_password(host, password, cmd_args):
    pid, fd = pty.fork()
    if pid == 0:
        # child - exec ssh
        os.execvp("ssh", ["ssh", "-F", "/dev/null", "-p", "22",
                          "-o", "StrictHostKeyChecking=no", 
                          "-o", "UserKnownHostsFile=/dev/null",
                          host] + cmd_args)
    else:
        # parent - send password
        output = b""
        time.sleep(2)
        attempts = 0
        while True:
            try:
                r, w, e = select.select([fd], [], [], 2.0)
                if r:
                    data = os.read(fd, 8192)
                    if not data:
                        break
                    output += data
                    # Check if it's asking for password
                    decoded = data.decode(errors='replace')
                    if ("password" in decoded.lower() or "assword" in decoded) and attempts < 1:
                        time.sleep(0.5)
                        os.write(fd, (password + "\n").encode())
                        attempts += 1
                else:
                    # Check if child is done
                    pid2, status = os.waitpid(pid, os.WNOHANG)
                    if pid2 != 0:
                        break
            except OSError:
                break
        try:
            os.waitpid(pid, 0)
        except:
            pass
        return output.decode(errors='replace')

result = ssh_with_password(target, password, cmd)
print(result)
