# rkvm
[![rkvm](https://img.shields.io/aur/version/rkvm)](https://aur.archlinux.org/packages/rkvm)

rkvm is a tool for sharing keyboard and mouse across multiple Linux machines.
It is based on a client/server architecture, where server is the machine controlling mouse and keyboard and relays events (mouse move, key presses, ...) to clients.

Switching between different clients is done by a configurable keyboard shortcut.

## Features
- TLS encrypted by default, backed by [rustls](https://github.com/rustls/rustls)
- Display server agnostic
- Low overhead

## Requirements
- Rust 1.48 and higher
- The uinput Linux kernel module, enabled by default in most distros. You can confirm that it's enabled in your distro by checking that `/dev/uinput` exists.
- libevdev development files (`sudo apt install libevdev-dev` on Debian/Ubuntu)
- Clang/LLVM (`sudo apt install clang` on Debian/Ubuntu)

## Configuration
After installation...
- Generate a certificate and private key using the `rkvm-certificate-gen` tool or provide your own from other sources.
- For server, place both the certificate and private key in `/etc/rkvm/certificate.pem` and `/etc/rkvm/key.pem` respectively.
- For client, place the certificate to `/etc/rkvm/certificate.pem`
- Create a config if you haven't done so already.
  Server:  
  ```
  # cp /etc/rkvm/server.example.toml /etc/rkvm/server.toml
  ```
  Client:
  ```
  # cp /etc/rkvm/client.example.toml /etc/rkvm/client.toml
  ```
  Do not edit the example configs, they will be overwritten by your package manager.
- **Change the password** and optionally reconfigure the network listen address and key bindings for switching clients
- Enable and start the systemd service.
  Server:
  ```
  # systemctl enable rkvm-server
  # systemctl start rkvm-server
  ```
  Client:
  ```
  # systemctl enable rkvm-client
  # systemctl start rkvm-client
  ```

## Why rkvm and not Barrier/Synergy?
The author of this program had a lot of problems with said programs, namely his keyboard layout (Czech) not being supported properly, which stems from the fact that the programs send characters which it then attempts to translate back into keycodes. rkvm takes a different approach to solving this problem and doesn't assume anything about your keyboard layout -- it sends raw keycodes only.

Additionally, rkvm doesn't even know or care about X, Wayland or any display server that might be in use, because it uses the uinput API with libevdev to read and generate input events.

Regardless, if you want a working and stable solution for crossplatform keyboard and mouse sharing, you should probably use either of the above mentioned programs for the time being.

## Limitations
- Only keyboard and relative mouse events work (no support for touchpads or other absolutely positioned devices)
- Linux only

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
