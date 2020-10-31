# rkvm
rkvm is a tool for sharing keyboard and mouse across multiple Linux computers.
It is based on a client/server architecture, where server is the machine controlling mouse and keyboard and relays events (mouse move, key presses, ...) to clients.

Switching between different clients is done by a configurable keyboard shortcut.

## Features
- TLS encrypted by default, backed by OpenSSL
- Display server agnostic
- Low overhead

## Requirements
- The uinput Linux kernel module, enabled by default in most distros
- libevdev
- Rust 1.48 and higher

## Building
Run `cargo build --release`. 
Note that you need to have libevdev installed on your system, otherwise the build will fail.

## Generating certificates
The repo contains a simple Rust program, `certificate-gen`, to aid certificate generation. 
Run `cargo run certificate-gen -- --help` to see and usage.

## Setting up
First, build the project and generate certificates. Client accepts certificates both in PEM and DER formats.
You either need to run either of the programs as root or make `/dev/uinput` accessible by the user it runs as.

By default, the programs reads their config files from /etc/rkvm/{server,client}.toml, this can be changed by passing the path as the first command line parameter.

The [example](example) directory contains example configurations and systemd service files.

## Why rkvm and not Barrier/Synergy?
The author of this program had a lot of problems with said programs, namely his keyboard layout (Czech) not being supported properly, which stems from the fact that the programs send characters which it then attempts to translate back into keycodes. rkvm takes a different approach to solving this problem and doesn't assume anything about your keyboard layout -- it sends raw keycodes only.

Additionally, rkvm doesn't even know or care about X, Wayland or any display server that might be in use, because it uses the uinput API with libevdev to read and generate input events.

Regardless, if you want a working and stable solution for crossplatform keyboard and mouse sharing, you should probably use either of the above mentioned programs for the time being.

## Limitations
- Only keyboard and relative mouse events work (that is, can be forwarded to clients)
- No Windows or macOS support
- No way to disable event forwarding on per user basis

Regardless, PRs are welcome.

## License
[MIT](LICENSE)