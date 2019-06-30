# NetHex
A small rust utility for dumping data-layer network traffic

[![Build Status](https://travis-ci.org/JackNewman12/NetHex.svg?branch=master)](https://travis-ci.org/JackNewman12/NetHex)
[![FOSSA Status](https://app.fossa.io/api/projects/git%2Bgithub.com%2FJackNewman12%2FNetHex.svg?type=shield)](https://app.fossa.io/projects/git%2Bgithub.com%2FJackNewman12%2FNetHex?ref=badge_shield)
![License: MIT](https://img.shields.io/badge/License-MIT-brightgreen.svg)

```
./net_hex --help
NetHex 0.7.0
Jack Newman jacknewman12@gmail.com
A small utility for reading / writing directly to a network interface

USAGE:
    net_hex [OPTIONS] [ARGS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -b, --blacklist <rx_blacklist_filter>    Only print Rx packets that do NOT match this regex filter
    -c, --count <rx_count>                   Number of packet to receive before exiting [default: -1]
    -f, --filter <rx_filter>                 Only print Rx packets that match this regex filter
    -t, --timeout <rx_timeout>               Time to receive for before exiting
    -r, --rate <tx_rate>                     Rate to transmit (Packets Per Second)
    -s, --send <tx_send>                     Number of packet to transmit [default: 1]

ARGS:
    <interface>    The network interface to use
    <bytes>        The hex bytes to send over the network


```


## Sending a packet
```
./net_hex eth0 112233445566778899AABBCCDDEEFF -c 0 -s 100 -r 50
Sending bytes: [11, 22, 33, 44, 55, 66, 77, 88, 99, AA, BB, CC, DD, EE, FF]
Sending bytes: [11, 22, 33, 44, 55, 66, 77, 88, 99, AA, BB, CC, DD, EE, FF]
Sending bytes: [11, 22, 33, 44, 55, 66, 77, 88, 99, AA, BB, CC, DD, EE, FF]
.. etc ..
```
* `--count 0` to not listen after transmitting
* `--send 100` send 100 of these packets
* `--rate 50` send 50 per second


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
* `--count 1` only grab one packet before exiting

### Filtering
Whitelist and blacklist filtering can be applied to the hex data. The filter is performed on the hex string only, not the ASCII area.
Whitespace and newlines are ignored in the filter.

* `--filter "77 77 88889999 AA"` Must contain this hex data. Whitespace ignored. Notice how this match crosses both lines
* `--blacklist "123456ABC"` Must not contain this hex data
```
 ./net_hex lo -f "77 77 88889999 AA" -c 1
[2019-06-30T08:17:24Z INFO  net_hex] Recv Packet
00000000  11 11 22 22 33 33 44 44 55 55 66 66 77 77 88 88  | ◄◄""33DDUUffwwêê |
00000010  99 99 AA AA BB BB CC CC DD DD EE EE FF FF        | ÖÖ¬¬╗╗╠╠▌▌εε..   |

```

## Logging
As of v0.5.0 most of the printing has been turned into an env_logger. While the debugging features are nice, disabling prints allows for a large 20x performance bonus. Logging can be enabled/disable using an enviroment variable. 
```
LOG=DEBUG ./nethex .........
LOG=WARN  ./nethex .........
```


## License
[![FOSSA Status](https://app.fossa.io/api/projects/git%2Bgithub.com%2FJackNewman12%2FNetHex.svg?type=large)](https://app.fossa.io/projects/git%2Bgithub.com%2FJackNewman12%2FNetHex?ref=badge_large)
