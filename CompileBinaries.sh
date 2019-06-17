echo "x64 build"
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
mv ./target/x86_64-unknown-linux-musl/release/net_hex ./net_hex_x64
strip ./net_hex_x64

echo "ARM build"
rustup target add armv7-unknown-linux-musleabihf
wget -nc https://toolchains.bootlin.com/downloads/releases/toolchains/armv7-eabihf/tarballs/armv7-eabihf--musl--stable-2018.11-1.tar.bz2
tar --skip-old-files -xf armv7-eabihf--musl--stable-2018.11-1.tar.bz2

RUSTFLAGS="-C linker=./armv7-eabihf--musl--stable-2018.11-1/bin/arm-buildroot-linux-musleabihf-ld" cargo build --release --target armv7-unknown-linux-musleabihf
mv ./target/armv7-unknown-linux-musleabihf/release/net_hex ./net_hex_arm
./armv7-eabihf--musl--stable-2018.11-1/bin/arm-buildroot-linux-musleabihf-strip ./net_hex_arm
