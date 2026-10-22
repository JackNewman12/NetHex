# NetHex
A small rust utility for dumping data-layer network traffic

[![Rust](https://github.com/JackNewman12/NetHex/actions/workflows/rust.yml/badge.svg)](https://github.com/JackNewman12/NetHex/actions/workflows/rust.yml)
![License: MIT](https://img.shields.io/badge/License-MIT-brightgreen.svg)

```
./nethex --help
A small utility for reading / writing directly to a network interface

Usage: nethex [OPTIONS] [interface] [bytes]

Arguments:
  [interface]  The network interface to use
  [bytes]      The hex bytes to send over the network

Options:
  -c, --count <RX_COUNT>                 Packets to receive before exiting [default: no limit]
  -t, --timeout <RX_TIMEOUT>             Seconds to receive for. Fails if --count is not reached
  -f, --filter <RX_FILTER>               Only print Rx packets that match this regex filter
  -b, --blacklist <RX_BLACKLIST_FILTER>  Only print Rx packets that do NOT match this regex filter
  -s, --send <TX_SEND>                   Number of packet to transmit [default: 1]
  -r, --rate <TX_RATE>                   Rate to transmit (Packets Per Second)
  -R, --raw                              Raw Hex Printing
  -T, --time                             Raw Hex Printing with timestamps
  -F, --file <TX_FILE>                   Inject Hex Packets from File
  -S, --stdin                            Inject Hex Packets from stdin
  -h, --help                             Print help
  -V, --version                          Print version
```


## Sending a packet
```
./nethex eth0 112233445566778899AABBCCDDEEFF -c 0 -s 100 -r 50
[00:00:02] ███████████████████████████     100/100     100% 49.997221/s 00:00:00
```
* `--count 0` to not listen after transmitting
* `--send 100` send 100 of these packets
* `--rate 50` send 50 per second

### Sending from File or Stdin
```
cat mydata.txt | ./nethex eth0 -c 0 --stdin
```
* `--stdin` will pipe any hex strings into the network interface, newline separated. Whitespace within a line and blank lines are ignored
Useful for fuzz testing a network interface / device

```
./nethex eth0 -c 0 --file mydata.txt
```
* `--file` will pipe any hex strings from a file into the network interface, newline separated
* Anything after a `#` is a comment
* `??` in place of a byte sends a new random byte every time, e.g. `ffffffffffff 020000000001 88b5 ?? ??`

To replay a pcap (needs `tshark` and `jq`):
```
tshark -r capture.pcap -T json -x | jq -r '.[]._source.layers.frame_raw[0]' | ./nethex eth0 -c 0 --stdin
```

## Monitoring a network interface
```
./nethex eth0 -c 1
[2026-10-22T11:30:05Z INFO  nethex] 
    0000:   ff ff ff ff ff ff 02 00 00 00 00 01 88 b5 de ad   ................
    0010:   be ef 00 11 22 33 44 55 66 77 88 99 aa bb cc dd   ...."3DUfw......
    0020:   ee ff                                             ..
```
* `--count 1` only grab one packet before exiting
* `--raw` print each packet as a single hex string instead
* `--time` same as `--raw`, with the seconds since start in front: `0.301025 ffffffffffff...`

### Scripting
`./nethex eth0 -c 3 -t 0.5` exits with code 1 if 3 packets did not arrive within half a second.
A `Sent N packets, received M` summary goes to stderr on exit.

### Filtering
Whitelist and blacklist filtering can be applied to the hex data. The filter is performed on the hex string only, not the ASCII area.
Whitespace and newlines are ignored in the filter.

* `--filter "77 77 88889999 AA"` Must contain this hex data. Whitespace ignored. Notice how this match crosses both lines
* `--blacklist "123456ABC"` Must not contain this hex data
```
 ./nethex lo -f "77 77 88889999 AA" -c 1
[2026-10-22T11:30:47Z INFO  nethex] 
    0000:   11 11 22 22 33 33 44 44 55 55 66 66 77 77 88 88   ..""33DDUUffww..
    0010:   99 99 aa aa bb bb cc cc dd dd ee ee ff ff         ..............
```

## Logging
As of v0.5.0 most of the printing has been turned into an env_logger. While the debugging features are nice, disabling prints allows for a large 20x performance bonus. Logging can be enabled/disable using an environment variable. 
```
LOG=DEBUG ./nethex .........
LOG=WARN  ./nethex .........
```
