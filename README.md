# NetHex
A small rust utility for dumping data-layer network traffic

[![Build Status](https://travis-ci.org/JackNewman12/NetHex.svg?branch=master)](https://travis-ci.org/JackNewman12/NetHex)

```
./net_hex --help
NetHex 0.1.0
Jack Newman
A small utility for reading / writing directly to a network interface

USAGE:
    net_hex [FLAGS] [OPTIONS] <interface> [bytes]

FLAGS:
    -h, --help       Prints help information
    -l, --list       List network interfaces
    -V, --version    Prints version information

OPTIONS:
    -c, --count <count>        Number of packet to receive before exiting [default: -1]
    -t, --timeout <timeout>    Timeout before exiting the program. Default no timeout

ARGS:
    <interface>    The network interface to send/read from
    <bytes>        A hex string of raw bytes to send to the interface e.g. 11EE22FF
```


## Sending a packet
```
./net_hex eth0 112233445566778899AABBCCDDEEFF -c 0
Sending bytes: [11, 22, 33, 44, 55, 66, 77, 88, 99, AA, BB, CC, DD, EE, FF]
```
`--count 0` to not listen after transmitting

Followed by a hex string of the bytes to transmit.

## Monitoring an network interface
```
./net_hex eth0 -c 1
----- Recv Packet -----
00000000  2A 1E 5F B3 8E E3 1A 8C 8C E9 2B 00 08 00 45 00  | *▲_│Ä∏→îîθ+.◘.E. |
00000010  00 28 CB 09 40 00 31 06 2C 45 B9 15 D8 A5 C0 A8  | .(╦○@.1♠,E╣§╪Ñ└¿ |
00000020  00 1E F3 B1 D5 CD A7 29 7E 51 D6 F6 B7 DE 50 10  | .▲≤▒╒═º)~Q╕÷╖▐P► |
00000030  00 80 DF 02 00 00       
```
