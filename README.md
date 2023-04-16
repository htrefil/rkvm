# rkvm
rkvm is a tool for sharing keyboard and mouse across multiple Linux and Windows machines.
It is based on a client/server architecture, where server is the machine controlling mouse and keyboard and relays events (mouse move, key presses, ...) to clients.

Switching between different clients is done by a configurable keyboard shortcut.

## Features
- TLS encrypted by default, backed by OpenSSL on Linux and SChannel on Windows (should be already installed on your machine by default)
- Display server agnostic
- Low overhead

## Requirements
- Rust 1.48 and higher

## Linux requirements
- The uinput Linux kernel module, enabled by default in most distros. You can confirm that it's enabled in your distro by checking that `/dev/uinput` exists.
- libevdev development files (`sudo apt install libevdev-dev` on Debian/Ubuntu)
- OpenSSL
- Clang/LLVM (`sudo apt install clang` on Debian/Ubuntu)

## Installation
1. First, build the project.

       $ cargo build --release

   Note that you need to have libevdev installed on your system, otherwise the build will fail.

2. Generate server certificates. The repo contains a simple Rust program, `certificate-gen`, to aid certificate generation. To see usage, run:

       $ cargo run --bin certificate-gen -- --help

   For example, where your server is *my-server-name* and your local network is *example.lan*, you might generate certificates with:

       $ mkdir cert/
       $ target/release/certificate-gen cert/my-server-name.p12 \
           cert/my-server-name_cert.pem \
           cert/my-server-name_key.pem \
           --dns-names my-server-name.example.lan

3. Install the release to its destination directory. `/opt/rkvm` is a good choice for Linux:

       $ sudo cp -r target/release /opt/rkvm

4. On Linux, you either need to run the programs as root or make `/dev/uinput` accessible by the user it runs as. For example, if the user belongs to the *rkvm* group, you could set `/dev/uinput` writeable by it:

       $ sudo chgrp rkvm /dev/uinput
       $ sudo chmod g+rw /dev/uinput

5. Create config files. By default, the program reads their config files from /etc/rkvm/{server,client}.toml on Linux and C:/rkvm/{server,client}.toml on Windows. This can be changed by passing the path as the first command line parameter.

   The [example](example) directory contains example configurations and systemd service files. If you are going to save your certificates in the same directory as the configuration, you will need to specify their full path in the config files.

6. Put the certificates in place. For example, on the server:

       $ sudo cp cert/* /etc/rkvm/

   Make sure the identify file is readable by the user that the server is running as.

   On a client:

       $ scp cert/my-server-name_cert.pem my-client1-name.example.lan:/etc/rkvm/

7. You are ready to go! Start the server with:

       $ /opt/rkvm/server

   Start the client with:

       $ /opt/rkvm/client

   Info-level logging is logged to the console.

8. If you want to start rkvm automatically, you can place the relevant systemd service file in /etc/systemd/system/. For example, on the server:

       $ sudo cp example/rkvm-server.service /etc/systemd/system/
       $ sudo chmod +x /etc/systemd/system/rkvm-server.service


## Why rkvm and not Barrier/Synergy?
The author of this program had a lot of problems with said programs, namely his keyboard layout (Czech) not being supported properly, which stems from the fact that the programs send characters which it then attempts to translate back into keycodes. rkvm takes a different approach to solving this problem and doesn't assume anything about your keyboard layout -- it sends raw keycodes only.

Additionally, rkvm doesn't even know or care about X, Wayland or any display server that might be in use, because it uses the uinput API with libevdev to read and generate input events.

Regardless, if you want a working and stable solution for crossplatform keyboard and mouse sharing, you should probably use either of the above mentioned programs for the time being.

## Limitations
- Only keyboard and relative mouse events work (that is, can be forwarded to clients)
- Clients only are supported on Windows, however, server support will be added in the future
- When Windows UAC is active the client needs elevated privileges to function properly. You may need to run the client in the System account (e.g. `psexec -sid client ...`)

## Project structure
- `server` - server application code
- `client` - client application code
- `input` - handles reading from and writing to input devices
- `net` - network protocol encoding and decoding
- `certificate-gen` - certificate generation tool

[Bincode](https://github.com/servo/bincode) is used for encoding of messages on the network and [Tokio](https://tokio.rs) as an asynchronous runtime.

## Contributions
All contributions, that includes both PRs and issues, are very welcome.

## License
[MIT](LICENSE)
