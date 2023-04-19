# rkvm
rkvm is a tool for sharing keyboard and mouse across multiple Linux machines.
It is based on a client/server architecture, where server is the machine controlling mouse and keyboard and relays events (mouse move, key presses, ...) to clients.

Switching between different clients is done by a configurable keyboard shortcut.

## Configuration
After installation, generate a certificate and private key using the `rkvm-certificate-gen` tool or provide your own certificate.  
- For server, place both the certificate and private key in `/etc/rkvm/certificate.pem` and `/etc/rkvm/key.pem` respectively.
- For client, place the certificate to `/etc/rkvm/certificate.pem`
- Finally, **change the password** and optionally reconfigure the network listen address and key bindings for switching clients

Note that the paths aren't hardcoded and can be changed in the config in `/etc/rkvm/{server,client}.toml`.

## Features
- TLS encrypted by default, backed by [rustls](https://github.com/rustls/rustls)
- Display server agnostic
- Low overhead

## Requirements
- Rust 1.48 and higher

## Linux requirements
- The uinput Linux kernel module, enabled by default in most distros. You can confirm that it's enabled in your distro by checking that `/dev/uinput` exists.
- libevdev development files (`sudo apt install libevdev-dev` on Debian/Ubuntu)
- Clang/LLVM (`sudo apt install clang` on Debian/Ubuntu)

## Why rkvm and not Barrier/Synergy?
The author of this program had a lot of problems with said programs, namely his keyboard layout (Czech) not being supported properly, which stems from the fact that the programs send characters which it then attempts to translate back into keycodes. rkvm takes a different approach to solving this problem and doesn't assume anything about your keyboard layout -- it sends raw keycodes only.

Additionally, rkvm doesn't even know or care about X, Wayland or any display server that might be in use, because it uses the uinput API with libevdev to read and generate input events.

Regardless, if you want a working and stable solution for crossplatform keyboard and mouse sharing, you should probably use either of the above mentioned programs for the time being.

## Limitations
- Only keyboard and relative mouse events work (that is, can be forwarded to clients)

## Project structure
- `rkvm-server` - server application code
- `rkvm-client` - client application code
- `rkvm-input` - handles reading from and writing to input devices
- `rkvm-net` - network protocol encoding and decoding
- `rkvm-certificate-gen` - certificate generation tool

[Bincode](https://github.com/servo/bincode) is used for encoding of messages on the network and [Tokio](https://tokio.rs) as an asynchronous runtime.

## Contributions
All contributions, that includes both PRs and issues, are very welcome.

## License
[MIT](LICENSE)
