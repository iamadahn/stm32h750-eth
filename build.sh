cargo build --release
arm-none-eabi-objcopy -O binary target/thumbv7em-none-eabihf/release/rust-h750-eth app.bin

