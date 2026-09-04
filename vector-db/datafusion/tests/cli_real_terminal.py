#!/usr/bin/env python3
"""Public real-terminal checks for the course SQL example."""

import os
import pty
import re
import select
import subprocess
import sys
import tempfile
import time


ANSI = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")


def normalized(transcript: bytes) -> str:
    return ANSI.sub("", transcript.decode("utf-8", errors="replace")).replace("\r", "")


class Terminal:
    def __init__(self, executable: str) -> None:
        master, slave = pty.openpty()
        environment = os.environ.copy()
        environment["TERM"] = "dumb"
        self.master = master
        self.output = bytearray()
        self.process = subprocess.Popen(
            [executable],
            stdin=slave,
            stdout=slave,
            stderr=slave,
            close_fds=True,
            env=environment,
        )
        os.close(slave)

    def send(self, text: str) -> None:
        os.write(self.master, text.encode())

    def wait_for_prompts(self, count: int, timeout: float = 15.0) -> str:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            text = normalized(self.output)
            if text.count("> ") >= count:
                return text
            ready, _, _ = select.select([self.master], [], [], 0.1)
            if ready:
                try:
                    self.output.extend(os.read(self.master, 65536))
                except OSError:
                    break
            if self.process.poll() is not None:
                break
        raise AssertionError(
            f"timed out waiting for prompt {count}; transcript:\n{normalized(self.output)}"
        )

    def finish(self) -> str:
        self.send("\\q\n")
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
            raise AssertionError(f"CLI did not exit; transcript:\n{normalized(self.output)}")
        while True:
            ready, _, _ = select.select([self.master], [], [], 0)
            if not ready:
                break
            try:
                chunk = os.read(self.master, 65536)
            except OSError:
                break
            if not chunk:
                break
            self.output.extend(chunk)
        os.close(self.master)
        self.master = -1
        if self.process.returncode != 0:
            raise AssertionError(
                f"CLI exited with {self.process.returncode}; transcript:\n"
                f"{normalized(self.output)}"
            )
        return normalized(self.output)


def assert_json_value(transcript: str, name: str, value: int) -> None:
    pattern = rf'\[\s*\{{\s*"{re.escape(name)}"\s*:\s*{value}\s*\}}\s*\]'
    if not re.search(pattern, transcript, re.DOTALL):
        raise AssertionError(f"missing JSON result for {name}; transcript:\n{transcript}")


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(f"usage: {sys.argv[0]} PATH_TO_SQL_EXAMPLE")
    executable = os.path.abspath(sys.argv[1])
    if not os.path.isfile(executable):
        raise SystemExit(f"SQL example does not exist: {executable}")

    with tempfile.NamedTemporaryFile("w", suffix=".sql", delete=False) as script:
        script.write("SELECT * FROM definitely_missing_table;\n")
        script.write("SELECT 42 AS continued_after_error;\n")
        script_path = script.name

    terminal = Terminal(executable)
    try:
        terminal.wait_for_prompts(1)
        terminal.send("\\pset format json\n")
        terminal.wait_for_prompts(2)
        terminal.send("SELECT 1 AS format_probe;\n")
        terminal.wait_for_prompts(3)
        terminal.send(f"\\i {script_path}\n")
        transcript = terminal.wait_for_prompts(4)
        transcript = terminal.finish()

        if "Output format is Json." not in transcript:
            raise AssertionError(f"output format did not change; transcript:\n{transcript}")
        assert_json_value(transcript, "format_probe", 1)
        if "definitely_missing_table" not in transcript:
            raise AssertionError(f"included error was not reported; transcript:\n{transcript}")
        assert_json_value(transcript, "continued_after_error", 42)
    finally:
        if terminal.process.poll() is None:
            terminal.process.kill()
            terminal.process.wait()
        if terminal.master >= 0:
            os.close(terminal.master)
        os.unlink(script_path)


if __name__ == "__main__":
    main()
