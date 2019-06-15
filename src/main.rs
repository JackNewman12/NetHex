extern crate hexplay;
extern crate pnet;

use pnet::datalink::Channel::Ethernet;
use pnet::datalink::{self, NetworkInterface};
use pnet::packet::ethernet::{EthernetPacket, MutableEthernetPacket};
use pnet::packet::{MutablePacket, Packet};

use std::env;

fn print_interfaces() {
    let list_of_interfaces = datalink::interfaces();
    for interface in list_of_interfaces {
        println!("{}", interface.name);
        for ipaddr in interface.ips {
            println!("  IP: {}", ipaddr);
        }
    }
}

// Invoke as echo <interface name>
fn main() {
    // let interface_name = env::args().nth(1).expect("Did not specify a network interface");
    let first_arg = env::args().nth(1);
    let interface_name = match first_arg {
        Some(x) => x,
        _ => {
            print_interfaces();
            std::process::exit(0);
        }
    };

    let interface_names_match = |iface: &NetworkInterface| iface.name == interface_name;

    // Find the network interface with the provided name
    let interfaces = datalink::interfaces();
    let interface = interfaces
        .into_iter()
        .filter(interface_names_match)
        .next()
        .unwrap();

    // Create a new channel, dealing with layer 2 packets
    let (mut tx, mut rx) = match datalink::linux::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Unhandled channel type"),
        Err(e) => panic!(
            "An error occurred when creating the datalink channel: {}",
            e
        ),
    };

    loop {
        match rx.next() {
            Ok(packet) => {
                // println!("{:?}", packet);
                use hexplay::HexViewBuilder;

                let view = HexViewBuilder::new(packet).row_width(16).finish();
                println!("{}", view);
                std::process::exit(0)

                // TX CODE TODO
                // let packet = EthernetPacket::new(packet).unwrap();

                // Constructs a single packet, the same length as the the one received,
                // using the provided closure. This allows the packet to be constructed
                // directly in the write buffer, without copying. If copying is not a
                // problem, you could also use send_to.
                //
                // The packet is sent once the closure has finished executing.
                // EthernetPacket::packet_size(&packet);

                // tx.build_and_send(1, packet.packet().len(),
                //     &mut |mut new_packet| {
                //         let mut new_packet = MutableEthernetPacket::new(new_packet).unwrap();

                //         // Create a clone of the original packet
                //         new_packet.clone_from(&packet);

                //         // Switch the source and destination
                //         new_packet.set_source(packet.get_destination());
                //         new_packet.set_destination(packet.get_source());
                // });
            }
            Err(e) => {
                // If an error occurs, we can handle it here
                panic!("An error occurred while reading: {}", e);
            }
        }
    }
}
