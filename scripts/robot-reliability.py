#!/usr/bin/env python3
"""Exclusive, bounded real-board protocol checks with an independent codec.

Requires pyserial 3.5. Quit Deskbot before use; this is never a soak reader.
Only opens the explicitly selected serial number and approved USB descriptor.
"""
import argparse
import json
import pathlib
import struct
import time
import zlib

import serial
from serial.tools import list_ports


def encode(raw):
    out = bytearray([0])
    mark, code = 0, 1
    for value in raw:
        if value == 0:
            out[mark] = code
            mark, code = len(out), 1
            out.append(0)
        else:
            out.append(value)
            code += 1
            if code == 255:
                out[mark] = code
                mark, code = len(out), 1
                out.append(0)
    out[mark] = code
    return bytes(out) + b'\0'


def decode(wire):
    out = bytearray()
    i = 0
    while i < len(wire):
        code = wire[i]
        i += 1
        if not code or i + code - 1 > len(wire):
            raise ValueError('Invalid COBS')
        out.extend(wire[i:i + code - 1])
        i += code - 1
        if code < 255 and i < len(wire):
            out.append(0)
    return bytes(out)


def raw_packet(kind, session, seq, payload=b'', flags=0):
    raw = struct.pack('<2sBBIIHH', b'DB', 1, kind, session, seq, len(payload), flags) + payload
    return raw + struct.pack('<I', zlib.crc32(raw))


def checker():
    return bytes(0xAA if x % 2 == 0 else 0x55 for x in range(1024))


def corners():
    out = bytearray(1024)
    def pixel(x, y):
        out[y // 8 * 128 + x] |= 1 << (y % 8)
    for x in range(128):
        pixel(x, 0)
        pixel(x, 63)
    for y in range(64):
        pixel(0, y)
        pixel(127, y)
    for y in range(2, 10):
        for x in range(2, 10):
            pixel(x, y)
    for y in range(54, 62):
        pixel(125, y)
    for x in range(117, 126):
        pixel(x, 61)
    return bytes(out)


class Robot:
    def __init__(self, identity):
        candidates = [p for p in list_ports.comports() if p.vid == 0x2e8a and p.pid == 3 and p.serial_number == identity]
        if not candidates:
            raise RuntimeError('Selected robot is not present')
        candidates.sort(key=lambda p: (not p.device.startswith('/dev/cu.'), p.device))
        self.port = serial.Serial(candidates[0].device, 115200, timeout=0.05, write_timeout=1, exclusive=True)
        self.port.dtr = True
        self.session = (time.time_ns() & 0xffffffff) or 1
        self.seq = 1
        self.pending = bytearray()

    def receive(self, duration=0.45):
        replies = []
        until = time.monotonic() + duration
        while time.monotonic() < until:
            self.pending.extend(self.port.read(2048))
            while 0 in self.pending:
                i = self.pending.index(0)
                wire = bytes(self.pending[:i])
                del self.pending[:i + 1]
                if not wire:
                    continue
                raw = decode(wire)
                assert len(raw) >= 20 and raw[:3] == b'DB\x01'
                magic, version, kind, session, seq, length, flags = struct.unpack('<2sBBIIHH', raw[:16])
                assert len(raw) == 20 + length and flags == 0
                assert struct.unpack('<I', raw[-4:])[0] == zlib.crc32(raw[:-4])
                replies.append((kind, session, seq, json.loads(raw[16:-4])))
        return replies

    def command(self, kind, payload=b'', expected=0x21):
        self.seq += 1
        raw = raw_packet(kind, self.session, self.seq, payload)
        self.port.write(encode(raw))
        replies = self.receive()
        match = [r for r in replies if r[0:3] == (expected, self.session, self.seq)]
        assert len(match) == 1, 'Missing or duplicate reply'
        health = match[0][3]
        assert health.get('displayHealth') == 'ready', 'OLED health failed'
        if kind == 0x10:
            assert health['frameCrc'] == zlib.crc32(payload), 'Wrong board frame checksum'
            assert health['nonzeroBytes'] == sum(bool(b) for b in payload), 'Wrong board byte count'
        return raw, health

    def display(self, on=True, orientation=0):
        return self.command(0x11, json.dumps({'on': on, 'contrast': 224, 'orientation': orientation}).encode())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--serial', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--pattern', choices=['checker', 'corners'])
    parser.add_argument('--orientation', type=int, choices=[0, 180], default=0)
    parser.add_argument('--hold', type=int, default=0)
    args = parser.parse_args()
    robot = Robot(args.serial)
    results = []
    started = time.time()
    try:
        _, hello = robot.command(1, b'{"client":"reliability","protocol":1}', 2)
        assert hello['boardId'].lower() == args.serial.lower()
        assert (hello['width'], hello['height']) == (128, 64)
        robot.display(orientation=args.orientation)
        if args.pattern:
            robot.command(0x10, checker() if args.pattern == 'checker' else corners())
            until = time.monotonic() + args.hold
            while time.monotonic() < until:
                robot.command(0x20)
                time.sleep(1)
            results.append({'test': args.pattern, 'orientation': args.orientation, 'result': 'frame_checksum_confirmed', 'physicalReview': 'pending'})
        else:
            reference = corners()
            robot.command(0x10, reference)
            damaged = bytearray(raw_packet(0x10, robot.session, robot.seq + 1, reference))
            damaged[-1] ^= 1
            defects = {
                'corrupt_crc': encode(damaged),
                'truncated_packet': encode(raw_packet(0x10, robot.session, robot.seq + 1, reference)[:30]),
                'overlong_packet': b'\x01' * 2000 + b'\0',
                'invalid_cobs': b'\xff\x01\0',
                'reserved_flags': encode(raw_packet(0x10, robot.session, robot.seq + 1, reference, flags=1)),
                'wrong_session': encode(raw_packet(0x10, robot.session ^ 1 or 2, robot.seq + 1, bytes(1024))),
                'old_sequence': encode(raw_packet(0x10, robot.session, 1, bytes(1024))),
            }
            for name, wire in defects.items():
                robot.port.write(wire)
                assert not robot.receive(), f'{name}: invalid request was acknowledged'
                robot.command(0x10, reference)
                results.append({'test': name, 'result': 'rejected_and_valid_frame_recovered'})
            robot.port.write(encode(raw_packet(0x10, robot.session, robot.seq + 1, reference))[:23])
            assert not robot.receive(0.7)
            robot.command(0x10, reference)
            results.append({'test': 'partial_packet_timeout', 'result': 'valid_frame_recovered'})
            raw, health = robot.command(0x10, checker())
            robot.port.write(encode(raw))
            assert robot.receive() == [(0x21, robot.session, robot.seq, health)]
            robot.port.write(encode(raw_packet(0x10, robot.session, robot.seq, bytes(1024))))
            assert not robot.receive()
            robot.command(0x10, reference)
            results.append({'test': 'duplicate_and_conflicting_duplicate', 'result': 'cached_ack_and_conflict_rejected'})
            _, error_health = robot.command(0x20)
            robot.seq += 1
            robot.port.write(encode(raw_packet(0x10, robot.session, robot.seq, b'bad')))
            assert robot.receive() == [(0x22, robot.session, robot.seq, {'code': 'frame_size'})]
            robot.command(0x10, reference)
            results.append({'test': 'wrong_frame_size', 'result': 'explicit_error_and_recovery'})
            for orientation in [0, 180, 0]:
                robot.display(orientation=orientation)
                robot.command(0x10, corners())
            robot.display(on=False)
            robot.display(on=True)
            robot.command(0x10, reference)
            results.append({'test': 'orientation_and_off_on', 'result': 'acked', 'physicalReview': 'pending'})
            robot.command(0x12)
            results.append({'test': 'fallback_command', 'result': 'acked', 'physicalReview': 'pending'})
        report = {'startedAt': started, 'endedAt': time.time(), 'firmware': hello['firmware'], 'result': 'protocol_checks_passed', 'checks': results, 'boundary': 'Board I2C/frame ACKs are measured. Pixel appearance and physical cable/sleep tests require independent observation.'}
    except Exception as error:
        report = {'startedAt': started, 'endedAt': time.time(), 'result': 'failed', 'checks': results, 'failure': str(error)}
        raise
    finally:
        robot.port.close()
        path = pathlib.Path(args.output)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
