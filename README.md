![Umpire](/images/Umpire-0.4.png "Umpire")

# Umpire: Combat Quest of the Millennium

Welcome to Umpire, a military strategy game with modern sensibilities as well as
a proper respect for the past. It supports Linux and Windows; macOS may also work.

Umpire's gameplay is inspired by the classic game _Empire_ and its text-based
user interface is influenced by that of _Dungeon Crawl Stone Soup_.
Umpire is implemented using the Rust programming language, both as an exercise
in learning Rust, and in order to gain the benefits of speed, safety, and
ergonomics relative to comparable languages that Rust offers.

## Installation

Umpire can be installed from `crates.io` or from this repository. A working
installation of Rust is prerequisite to running Umpire. Please refer to
[the official Rust installation guide](https://www.rust-lang.org/en-US/install.html)
for guidance.

For systems with Rust and the Cargo package manager installed, the easiest way to
install Umpire is to run this command:

    cargo install umpire

This will install an `umpire` binary into Cargo's binary path. Running the `umpire`
binary will launch the game.

Cargo can also be installed from this repository:

     git clone https://github.com/joshhansen/Umpire.git && cd Umpire
    cargo run --release

This should grab the source code, build it, and run the game.

## Playing Umpire

Upon loading, Umpire shows a ridiculous ASCII art splash screen of a baseball
umpire. It has nothing to do with military strategy.

Running `umpire` with no arguments starts a local game including one human player and three AI players of varying difficulties.

If a hostname is provided, the client will attempt to connect to a server at the destination:

```bash
umpire example.com
```

The Message Log will indicate whose turn it is. When a turn begins,
the player is prompted with any necessary decisions.

Players control cities which can produce units, and control units which can
move, attack other units, attack and capture cities, and a few other special
functions.

A player wins when all enemy cities have been captured and all enemy units have
been destroyed.

Pressing 'x' engages Examine Mode which allows map tiles to be inspected. Pressing 'Enter' over a map tile can cancel a
unit's orders, clear a city's production, or go-to a particular tile or empty space.

### The Fog of War

A fog of war mechanic is implemented but can be disabled using the `--fog off` command line option.

### Wrapping

Wrapping can be turned on and off in both dimensions, but at the moment turning it off provokes a few bugs.

### Color support

An effort has been made to support a range of color palettes. These can be controlled using the `--colors` command line
flag. The 16 color palette is the best tested at present.

## Startup Options

All binaries included in Umpire respond to a wide range of command-line flags, visible when run with `--help`:

    cargo run --release -- --help


When running with Cargo, any options desired can be placed after the `--`. These include things such as
whether to enable fog of war, and how many players of which type to include in the game.

The same can be done with the `umpire` binary:

    umpire --help


## Server

A server is provided, allowing networked multiplayer using an RPC protocol. The server runs AIs and coordinates remote clients.

The server should be installed in the same path as the main binary. Run `umpired --help` for command-line options.

A sample SystemD service definition is included in the repository; see `server/umpired.service`.

## AI

The AI's were trained using reinforcement learning, specifically Q-Learning provided by `rsrl`.

Work has also been done on an AlphaGo Zero style self-trained model, though this is not yet included in the default slate of AIs.

The tools used to train the included AI algorithms are provided. Run `umpire-ai --help` for more information.

## Features

One Cargo feature is available: `"pytorch"`.

PyTorch is used for neural network training; however, it is difficult to install in some environments, so it is disabled by default. If you wish to enable it, pass `-F pytorch`:

```bash
cargo build --all -F pytorch
```

## Name

Why is Umpire called Umpire? Because it's silly, and it harks back to the game
that inspired it.

## History

* Umpire 0.5.0---networked multiplayer; basic AI; AI training framework

## License

Umpire is licensed under version 3 of the GNU General Public License (GPLv3).
See `LICENSE` for detailed license terms.


TODO: New release on crates.io - pytorch disabled
~~TODO: Double-check that cargo run runs client~~
