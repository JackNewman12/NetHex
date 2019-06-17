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
