import importlib.util
import os
from pathlib import Path
import pty
import select
import threading
import unittest


SPEC = importlib.util.spec_from_file_location("resetter_host", Path(__file__).resolve().parents[1] / "resetter.py")
HOST = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HOST)


class Device:
    def __init__(self, respond):
        self.master, self.slave = pty.openpty()
        self.port = os.ttyname(self.slave)
        self.respond = respond
        self.commands = []
        self.stopped = threading.Event()
        self.errors = []
        self.thread = threading.Thread(target=self.run, daemon=True)

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_):
        self.stopped.set()
        self.thread.join(timeout=1)
        os.close(self.master)
        os.close(self.slave)
        if self.errors:
            raise self.errors[0]

    def run(self):
        buffer = bytearray()
        try:
            while not self.stopped.is_set():
                if not select.select([self.master], [], [], 0.02)[0]:
                    continue
                buffer.extend(os.read(self.master, 256))
                while b"\n" in buffer:
                    command, _, rest = buffer.partition(b"\n")
                    buffer = bytearray(rest)
                    command = command.rstrip(b"\r").decode("ascii")
                    self.commands.append(command)
                    for packet in self.respond(command):
                        os.write(self.master, packet)
        except BaseException as error:
            self.errors.append(error)


class HostTests(unittest.TestCase):
    def test_identity_handshake_precedes_reset(self):
        def respond(command):
            if command == "PING":
                return [b"PONG kb-", b"resetter 1\r\n"]
            return [b"ACK RESET1\r\nDONE RESET1\r\n"]

        with Device(respond) as device:
            output = []
            HOST.execute(device.port, ["RESET1"], emit=output.append)
            self.assertEqual(device.commands, ["PING", "RESET1"])
            self.assertEqual(output, ["ACK RESET1", "DONE RESET1"])

    def test_two_channels_can_finish_in_the_opposite_order(self):
        def respond(command):
            if command == "PING":
                return [b"PONG kb-resetter 1\n"]
            if command == "BOOT1":
                return [b"ACK BOOT1\n"]
            return [b"ACK RESET2\nDONE RESET2\nDONE BOOT1\n"]

        with Device(respond) as device:
            output = []
            HOST.execute(device.port, ["BOOT1", "RESET2"], emit=output.append)
            self.assertEqual(device.commands, ["PING", "BOOT1", "RESET2"])
            self.assertEqual(output[-2:], ["DONE RESET2", "DONE BOOT1"])

    def test_a_different_serial_device_never_receives_reset(self):
        with Device(lambda _: [b"PONG something-else\n"]) as device:
            with self.assertRaises(TimeoutError):
                HOST.execute(device.port, ["RESET1"], timeout=0.1, emit=lambda _: None)
            self.assertEqual(device.commands, ["PING"])

    def test_busy_response_is_reported_as_an_error(self):
        def respond(command):
            return [b"PONG kb-resetter 1\n"] if command == "PING" else [b"ERR BUSY BOOT1\n"]

        with Device(respond) as device:
            with self.assertRaisesRegex(RuntimeError, "BUSY BOOT1"):
                HOST.execute(device.port, ["BOOT1"], emit=lambda _: None)

    def test_status_does_not_require_a_done_response(self):
        def respond(command):
            return [b"PONG kb-resetter 1\n"] if command == "PING" else [b"STATUS 1=LOW1 2=IDLE\n"]

        with Device(respond) as device:
            output = []
            HOST.execute(device.port, ["STATUS"], emit=output.append)
            self.assertEqual(output, ["STATUS 1=LOW1 2=IDLE"])

    def test_invalid_command_is_not_transmitted(self):
        with Device(lambda _: [b"PONG kb-resetter 1\n"]) as device:
            with self.assertRaises(ValueError):
                HOST.execute(device.port, ["RESET3"], emit=lambda _: None)
            self.assertEqual(device.commands, ["PING"])


if __name__ == "__main__":
    unittest.main()
