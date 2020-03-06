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

    git clone git@github.com:joshhansen/Umpire.git && cd Umpire
    cargo run --release

This should grab the source code, build it, and run the game.

## Playing Umpire
Upon loading, Umpire shows a ridiculous ASCII art splash screen of a baseball
umpire. It has nothing to do with military strategy.

Right now Umpire lacks an AI and can only be played as hotseat style
multiplayer. The Message Log will indicate whose turn it is. When a turn begins,
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
The full list of startup options can be seen thus:

    cargo run --release -- --help


Any options desired can be placed after the `--`. These include things such as
whether to enable fog of war, and how many players to include in the game.

The same can be done with the `umpire` binary:

    umpire --help


## Name
Why is Umpire called Umpire? Because it's silly, and it harks back to the game
that inspired it.

## License
Umpire is licensed under version 3 of the GNU General Public License (GPLv3).
See `LICENSE` for detailed license terms.
