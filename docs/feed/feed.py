#!/usr/bin/env python3
"""Read Pro Engineer's feed: subscribe, keep the subscription, print.

    python3 feed.py                    # the program on this machine
    python3 feed.py 192.168.1.42       # another machine, if its driver allowed it

Standard library only. Copy it into your own program: `Feed.poll()` never
blocks, so it sits in a drawing loop as it is. See README.md beside it.
"""

import json
import socket
import sys
import time

PORT = 9003
LEASE_S = 10


class Feed:
    def __init__(self, host="127.0.0.1", hz=20, name="python"):
        self.server = (host, PORT)
        self.ask = json.dumps({"magic": "acpe", "subscribe": {"hz": hz, "name": name}}).encode()
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.sock.bind(("127.0.0.1" if host.startswith("127.") else "0.0.0.0", 0))
        self.sock.setblocking(False)
        self.asked = 0.0
        self.last_sequence = -1
        self.standing = "asking"

    def poll(self):
        """Renew the subscription when due; return the newest message, or None."""
        now = time.monotonic()
        if now - self.asked >= LEASE_S / 3:
            try:
                self.sock.sendto(self.ask, self.server)
            except OSError:
                pass  # nothing listening yet; the next ask finds it
            self.asked = now
        newest = None
        while True:
            try:
                data, _ = self.sock.recvfrom(65535)
            except (BlockingIOError, ConnectionRefusedError, OSError):
                return newest
            try:
                message = json.loads(data)
            except ValueError:
                continue
            if message.get("magic") != "acpe":
                continue
            if "subscribed" in message:
                self.standing = "subscribed" if message["subscribed"] else "refused: " + message.get("reason", "")
                continue
            if message.get("schema") != 1:
                continue  # written against a newer feed; update this reader
            if message["sequence"] < self.last_sequence:
                continue  # arrived late
            self.last_sequence = message["sequence"]
            newest = message

    def close(self):
        self.sock.sendto(b'{"magic":"acpe","unsubscribe":true}', self.server)


if __name__ == "__main__":
    feed = Feed(sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1", hz=10, name="feed.py")
    said = None
    try:
        while True:
            message = feed.poll()
            if feed.standing != said:
                said = feed.standing
                print(said, flush=True)
            if message:
                advice = message["advice"][0]["text"] if message["advice"] else "—"
                print(f'{message["speed_kmh"]:5.0f} km/h  gear {message["gear"]:2d}  '
                      f'fuel {message["fuel_litres"]:5.1f} l  | {advice}', flush=True)
            time.sleep(0.02)
    except (KeyboardInterrupt, BrokenPipeError):
        pass
    finally:
        feed.close()
