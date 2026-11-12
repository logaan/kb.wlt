#!/usr/bin/env python3

import argparse
import glob
import os
import select
import sys
import termios
import time


COMMANDS = {"PING", "STATUS", "RESET1", "RESET2", "BOOT1", "BOOT2"}


def ports():
    return sorted(set(glob.glob("/dev/cu.usbmodem*") + glob.glob("/dev/ttyACM*")))


def write_all(descriptor, data, deadline):
    while data:
        remaining = deadline - time.monotonic()
        if remaining <= 0 or not select.select([], [descriptor], [], remaining)[1]:
            raise TimeoutError("Timed out writing to the reset controller")
        try:
            count = os.write(descriptor, data)
        except BlockingIOError:
            continue
        if count == 0:
            raise OSError("Reset controller disconnected")
        data = data[count:]


class Serial:
    def __init__(self, port):
        self.descriptor = os.open(port, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
        self.buffer = bytearray()
        self.original = None
        try:
            self.original = termios.tcgetattr(self.descriptor)
            settings = termios.tcgetattr(self.descriptor)
            settings[0] = 0
            settings[1] = 0
            settings[2] = termios.CS8 | termios.CREAD | termios.CLOCAL
            settings[3] = 0
            settings[4] = termios.B115200
            settings[5] = termios.B115200
            settings[6][termios.VMIN] = 0
            settings[6][termios.VTIME] = 0
            termios.tcsetattr(self.descriptor, termios.TCSANOW, settings)
            termios.tcflush(self.descriptor, termios.TCIFLUSH)
        except BaseException:
            os.close(self.descriptor)
            raise

    def close(self):
        if self.descriptor is not None:
            try:
                if self.original is not None:
                    termios.tcsetattr(self.descriptor, termios.TCSANOW, self.original)
            except OSError:
                pass
            finally:
                os.close(self.descriptor)
                self.descriptor = None

    def send(self, command, deadline):
        if command not in COMMANDS:
            raise ValueError("Unknown resetter command: " + command)
        write_all(self.descriptor, command.encode("ascii") + b"\n", deadline)

    def line(self, deadline):
        while True:
            if b"\n" in self.buffer:
                line, _, rest = self.buffer.partition(b"\n")
                self.buffer = bytearray(rest)
                return line.rstrip(b"\r").decode("ascii")
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not select.select([self.descriptor], [], [], remaining)[0]:
                raise TimeoutError("Timed out waiting for the reset controller")
            try:
                data = os.read(self.descriptor, 256)
            except BlockingIOError:
                continue
            if not data:
                raise OSError("Reset controller disconnected")
            self.buffer.extend(data)
            if len(self.buffer) > 1024:
                raise ValueError("Reset controller returned an oversized response")


def execute(port, commands, timeout=5.0, emit=print):
    serial = Serial(port)
    try:
        deadline = time.monotonic() + timeout
        serial.send("PING", deadline)
        while serial.line(deadline) != "PONG kb-resetter 1":
            pass
        pending = []
        deadline = time.monotonic() + timeout
        for command in commands:
            serial.send(command, deadline)
            pending.append(
                "PONG kb-resetter 1" if command == "PING"
                else "STATUS " if command == "STATUS"
                else "DONE " + command
            )
        while pending:
            line = serial.line(deadline)
            if line.startswith("ERR "):
                raise RuntimeError(line)
            emit(line)
            for index, expected in enumerate(pending):
                if line == expected or (expected == "STATUS " and line.startswith(expected)):
                    pending.pop(index)
                    break
    finally:
        serial.close()


def main():
    parser = argparse.ArgumentParser(description="Control two reset lines from a third XIAO nRF52840")
    parser.add_argument("commands", nargs="*", type=str.upper, default=["PING"])
    parser.add_argument("--port", help="Serial port; inferred when exactly one USB modem is present")
    parser.add_argument("--timeout", type=float, default=5.0)
    args = parser.parse_args()
    if args.commands == ["PORTS"]:
        for port in ports():
            print(port)
        return 0
    if not args.commands:
        args.commands = ["PING"]
    invalid = [command for command in args.commands if command not in COMMANDS]
    if invalid:
        parser.error("commands: PING STATUS RESET1 RESET2 BOOT1 BOOT2, or PORTS")
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    port = args.port
    if port is None:
        candidates = ports()
        if len(candidates) != 1:
            parser.error("specify --port; candidates: " + (", ".join(candidates) or "none"))
        port = candidates[0]
    try:
        execute(port, args.commands, args.timeout)
    except (OSError, TimeoutError, RuntimeError, ValueError) as error:
        print("error: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
