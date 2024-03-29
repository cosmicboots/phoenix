# Phoenix

Phoenix is a chunk-based file synchronization platform using a custom binary
protocol that sits on-top of the [Noise Protocol](https://noiseprotocol.org/).

> **_NOTE_**: This application is currently targeted at Linux x86-64 bit
> systems. No other platforms should expect a working application.

## The State of File Synchronization

Current file synchronization tools tend to fit into two buckets:

1. Manual/on-demand synchronization
2. Centralized event based file synchronization

The first group is very well developed and absolutely amazing tools exist for
it. `rsync(1)` is a classic example of this.

The second bucket as a lot of very common tools in it, but none of them are
really good. These would be services like Google Drive, Dropbox, OneDrive, and
even self-hosted tools like Nextcloud. The current solutions in the field are
poorly built and seem to think of file transfers as an afterthought to
pre-existing web standards. 

## Phoenix to the Rescue!

Phoenix focuses on the second bucket listed above: centralized event based file
synchronization. 

To solve the issues with current tooling,the solution as to be approached with
the number one priority being securely and robustly sending files. For this
concept to work, an entire new ecosystem will have to be created; client,
protocol, and server included.

## Development/Contributing

> **_NOTE_**: The commands in this section require a working rust tool chain to
> be setup on your system. Instructions on how to do this vary between systems,
> but [rustup](https://rustup.rs/) is a good starting place.

### Setting Up The Program

Once you've cloned the repository and have the rust tool chain setup, you can
start messing around with the project.

Phoenix can run in two different modes:

1. Server Mode
2. Client Mode

Both of these will require configuration files. The default configuration
values can be gotten by running the following commands:

```
cargo run -- dump-config
cargo run -- dump-config --server
```

Custom keys will need to be created with 
```
cargo run -- gen-key
```

After the configuration files are written:

1. Start the server
2. Start the client with a directory to watch as the command argument

At this point the client and server should be talking to each other.

### Installing / Building Release Binary

To build the release binary, simply invoke cargo build with the release flag:
```shell
cargo build --release
```

To install the application:
```shell
cargo install --path .
```

Uninstall:
```shell
cargo uninstall
```

### Code Documentation

The code should be fairly well documented and could be generated and viewed
using `cargo`:
```
cargo doc --open
```

If it looks like importation documentation is missing, open an issue or submit
a PR!
