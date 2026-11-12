// Read Pro Engineer's feed from Node: subscribe, keep the subscription, print.
//
//     node feed.mjs                 # the program on this machine
//     node feed.mjs 192.168.1.42    # another machine, if its driver allowed it
//
// No packages. See README.md beside it.

import dgram from 'node:dgram';

const PORT = 9003;
const LEASE_MS = 10_000;
const host = process.argv[2] ?? '127.0.0.1';
const ask = Buffer.from(JSON.stringify({ magic: 'acpe', subscribe: { hz: 10, name: 'feed.mjs' } }));
const socket = dgram.createSocket('udp4');
let lastSequence = -1;

socket.on('message', (data) => {
  let message;
  try { message = JSON.parse(data); } catch { return; }
  if (message.magic !== 'acpe') return;
  if ('subscribed' in message) {
    console.log(message.subscribed ? `subscribed, ${message.hz} a second` : `refused: ${message.reason}`);
    return;
  }
  if (message.schema !== 1 || message.sequence < lastSequence) return;
  lastSequence = message.sequence;
  const advice = message.advice[0]?.text ?? '—';
  console.log(`${message.speed_kmh.toFixed(0).padStart(5)} km/h  gear ${message.gear}  | ${advice}`);
});
socket.on('error', () => {}); // nothing listening yet: the next ask finds it
process.stdout.on('error', () => process.exit(0)); // piped into head

const renew = () => socket.send(ask, PORT, host);
socket.bind(0, host.startsWith('127.') ? '127.0.0.1' : '0.0.0.0', renew);
const timer = setInterval(renew, LEASE_MS / 3);

process.on('SIGINT', () => {
  clearInterval(timer);
  socket.send('{"magic":"acpe","unsubscribe":true}', PORT, host, () => process.exit(0));
});
