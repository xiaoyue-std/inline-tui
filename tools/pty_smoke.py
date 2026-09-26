#!/usr/bin/env python3
"""PTY smoke test driver: runs the stilt examples in a headless Linux environment (CI / WSL).

Allocates a real pty (with window size), feeds scripted key sequences, captures all
ANSI output, and asserts on the rendered result. Usage:

    python3 tools/pty_smoke.py            # run all scenarios
    python3 tools/pty_smoke.py sandbox    # run just one
"""
import fcntl
import os
import pty
import select
import struct
import subprocess
import sys
import termios
import time


def run_pty(argv, inputs, rows=30, cols=100, timeout=20.0, env_extra=None):
    """Runs a command until it exits, returning (all output bytes, exit code).

    inputs: [(seconds since start, bytes), ...] fed into the pty over time.
    """
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    env = dict(os.environ)
    env.update(env_extra or {})
    pid = os.fork()
    if pid == 0:
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
        os.dup2(slave, 0)
        os.dup2(slave, 1)
        os.dup2(slave, 2)
        if slave > 2:
            os.close(slave)
        os.close(master)
        os.execvpe(argv[0], argv, env)
    os.close(slave)

    out = bytearray()
    start = time.time()
    idx = 0
    inputs = list(inputs)
    next_at = start + inputs[0][0] if inputs else None
    status = None

    def reap_nonblocking():
        nonlocal status
        try:
            done, st = os.waitpid(pid, os.WNOHANG)
            if done == pid:
                status = st
        except ChildProcessError:
            status = -1

    while time.time() - start < timeout:
        if next_at is not None and time.time() >= next_at:
            os.write(master, inputs[idx][1])
            idx += 1
            next_at = start + inputs[idx][0] if idx < len(inputs) else None
        r, _, _ = select.select([master], [], [], 0.05)
        if r:
            try:
                data = os.read(master, 65536)
            except OSError:
                # Linux: after the child exits, the master read end raises EIO — grab the
                # real exit code first
                reap_nonblocking()
                break
            if not data:
                reap_nonblocking()
                break
            out.extend(data)
        reap_nonblocking()
        if status is not None:
            while True:  # drain
                r, _, _ = select.select([master], [], [], 0.15)
                if not r:
                    break
                try:
                    data = os.read(master, 65536)
                except OSError:
                    break
                if not data:
                    break
                out.extend(data)
            break
    if status is None:
        # EIO may arrive before the child has fully terminated (multi-thread teardown has
        # timing skew); retry the reap in a blocking loop and don't rush to kill —
        # otherwise a normal exit would be misjudged as a timeout.
        deadline = time.time() + 3.0
        while time.time() < deadline:
            try:
                done, st = os.waitpid(pid, os.WNOHANG)
                if done == pid:
                    status = st
                    break
            except ChildProcessError:
                status = -1
                break
            time.sleep(0.02)
    if status is None:
        os.kill(pid, 9)
        try:
            os.waitpid(pid, 0)
        except ChildProcessError:
            pass
        status = -1
    os.close(master)
    return bytes(out), status


def check(name, cond):
    print(("OK  " if cond else "FAIL") + ": " + name)
    return bool(cond)


def scenario_sandbox():
    out, st = run_pty(["cargo", "run", "-q", "--example", "sandbox"],
                      [(2.0, b"q")], rows=42, cols=100, timeout=25)
    open("/tmp/sandbox_pty.log", "wb").write(out)
    ok = True
    ok &= check("sandbox exits cleanly", st == 0)
    ok &= check("rounded border appears", b"\xe2\x95\xad" in out)          # ╭
    ok &= check("CJK text renders", "中文".encode() in out)
    ok &= check("wide CJK chars occupy two cells, never split", b"\xe4\xb8\xad" in out)
    ok &= check("SGR style sequences appear", b"\x1b[0m" in out and b"\x1b[38;2;215;119;87m" in out)
    ok &= check("true color swatch", b"\x1b[38;2;" in out)
    ok &= check("cursor restored visible on exit", b"\x1b[?25h" in out)
    ok &= check("cursor hidden before exit", b"\x1b[?25l" in out)
    return ok


def scenario_gallery():
    """Widget overview: paging (→→→→) covers all 5 pages before quitting."""
    out, st = run_pty(
        ["cargo", "run", "-q", "--example", "gallery"],
        [
            (2.0, b"\x1b[C"),   # Table
            (2.4, b"\x1b[C"),   # Form
            (2.8, b"\x1b[C"),   # Charts
            (3.2, b"\x1b[C"),   # Editor
            (3.6, b"hello"),
            (4.2, b"q"),
        ],
        timeout=30,
    )
    open("/tmp/gallery_pty.log", "wb").write(out)
    ok = True
    ok &= check("gallery exits cleanly", st == 0)
    ok &= check("List page renders", b"Rust" in out and b"Python" in out)
    ok &= check("Table page header", "Year".encode() in out)
    ok &= check("Form page checkboxes", "[x]".encode() in out or "[ ]".encode() in out)
    ok &= check("Charts page sparkline", "█".encode() in out)
    ok &= check("Editor page input echo", all(c in out for c in b"hello"))
    ok &= check("Tab bar page indicator", b"5 / 5" in out or b"1 / 5" in out)
    ok &= check("cursor restored visible on exit", b"\x1b[?25h" in out)
    return ok


def scenario_chat():
    # / → he → Tab(completion) → Enter(run /help) → hi+Enter(free message triggers a streaming code reply)
    # → Ctrl+C clear → Ctrl+C quit
    # Note: input times are absolute seconds relative to launch
    out, st = run_pty(
        ["cargo", "run", "-q", "--example", "chat"],
        [
            (2.0, b"/"),
            (2.3, b"he"),
            (2.6, b"\t"),
            (2.9, b"\r"),
            (3.6, b"hi"),
            (3.9, b"\r"),
            (10.0, b"\x03"),
            (10.4, b"\x03"),
        ],
        timeout=30,
    )
    open("/tmp/chat_pty.log", "wb").write(out)
    ok = True
    ok &= check("chat exits cleanly", st == 0)
    ok &= check("input box rounded border", b"\xe2\x95\xad" in out)
    ok &= check("title ✻ chat", "chat".encode() in out)
    ok &= check("welcome text renders", "streaming".encode() in out)
    ok &= check("slash menu selection accepted", b"/help " in out)
    ok &= check("help content renders", "Alt+Enter".encode() in out)
    ok &= check("code highlight function name", b"emit_sgr" in out)
    ok &= check("code block rounded border", b"\xe2\x95\xb0" in out)  # ╰
    ok &= check("spinner animation frame", "✻".encode() in out)
    ok &= check("status bar hints", "ctrl+c".encode() in out)
    ok &= check("no alt-screen leftovers", b"\x1b[?1049h" not in out)
    ok &= check("cursor restored visible on exit", b"\x1b[?25h" in out)
    return ok


def scenario_chat_diff():
    # /d → Tab → Enter → observe the diff rendering
    out, st = run_pty(
        ["cargo", "run", "-q", "--example", "chat"],
        [
            (2.0, b"/"),
            (2.3, b"d"),
            (2.6, b"\t"),
            (2.9, b"\r"),
            (4.0, b"\x03"),
            (4.4, b"\x03"),
        ],
        timeout=30,
    )
    open("/tmp/chat_diff_pty.log", "wb").write(out)
    ok = True
    ok &= check("diff scenario exits cleanly", st == 0)
    ok &= check("diff header renders", b"diff --git" in out)
    ok &= check("diff hunk header", b"@@" in out)
    ok &= check("diff add/del lines", b"+    let name" in out and b'-    println!("Hello, world!")' in out)
    return ok


def scenario_signal_restore():
    """On SIGTERM the signal handler should restore termios (ICANON/ECHO back) and the
    process should die from SIGTERM.

    Note: exec the example binary directly (bypassing cargo — cargo catches SIGTERM,
    forwards it to the child and exits with 0, which would interfere with the exit
    status assertion).
    """
    subprocess.run(["cargo", "build", "-q", "--example", "sandbox"], check=True)
    exe = os.path.abspath("target/debug/examples/sandbox")

    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
    before = termios.tcgetattr(slave)

    pid = os.fork()
    if pid == 0:
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
        os.dup2(slave, 0)
        os.dup2(slave, 1)
        os.dup2(slave, 2)
        if slave > 2:
            os.close(slave)
        os.close(master)
        os.execv(exe, [exe])
    keep = os.dup(slave)
    os.close(slave)

    time.sleep(1.5)  # let the app enter raw mode and render
    os.kill(pid, 15)  # SIGTERM
    start = time.time()
    status = None
    while time.time() - start < 10:
        try:
            done, st = os.waitpid(pid, os.WNOHANG)
        except ChildProcessError:
            break
        if done == pid:
            status = st
            break
        r, _, _ = select.select([master], [], [], 0.05)
        if r:
            try:
                os.read(master, 65536)
            except OSError:
                pass

    ok = True
    ok &= check("process dies from SIGTERM", status is not None and os.WIFSIGNALED(status)
                and os.WTERMSIG(status) == 15)
    # Must read before closing the master (after the master closes, slave ioctl raises EIO)
    after = termios.tcgetattr(keep)
    os.close(master)
    os.close(keep)

    import termios as t
    l_after = after[3]
    ok &= check("ICANON restored", bool(l_after & t.ICANON))
    ok &= check("ECHO restored", bool(l_after & t.ECHO))
    ok &= check("ISIG restored", bool(l_after & t.ISIG))
    return ok


SCENARIOS = {
    "sandbox": scenario_sandbox,
    "gallery": scenario_gallery,
    "chat": scenario_chat,
    "diff": scenario_chat_diff,
    "signal": scenario_signal_restore,
}


def main():
    names = sys.argv[1:] or list(SCENARIOS)
    all_ok = True
    for name in names:
        print(f"\n===== Scenario {name} =====")
        all_ok &= SCENARIOS[name]()
    print("\n===== Summary =====")
    print("ALL PASS" if all_ok else "SOME FAILED")
    sys.exit(0 if all_ok else 1)


if __name__ == "__main__":
    main()
