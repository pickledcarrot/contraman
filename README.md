# contraman

`contraman` is a small command-line password manager written in Rust. It stores
passwords in the operating system's native credential store and keeps a separate
local index of entry names so saved items can be listed later.

The actual password values are not written to the project directory by the app.
They are stored through the [`keyring`](https://crates.io/crates/keyring) crate,
using the platform keychain where available:

- macOS: Keychain
- Windows: Credential Manager
- Linux: Secret Service-compatible keyring

## What It Does

`contraman` manages named password entries:

- `set` stores or updates a password for a name.
- `get` retrieves a password, prints it, and copies it to the clipboard.
- `list` prints the known entry names.
- `remove` deletes an entry from the keychain and from the local name index.

The program uses the constant service name `contraman`, so entries are stored in
the OS credential store under that service and the provided entry name.

## Usage

Build and run it with Cargo:

```sh
cargo run -- set github
cargo run -- get github
cargo run -- github
cargo run -- list
cargo run -- remove github
```

After installing the binary, the same commands are available directly:

```sh
contraman set github
contraman get github
contraman github
contraman list
contraman remove github
```

`contraman <name>` is shorthand for `contraman get <name>`.

This project also builds a `getpass` binary alias. It runs the same CLI, so this
works after installation:

```sh
getpass github
```

## Installation

Install both command names from this checkout with:

```sh
cargo install --path .
```

Cargo installs binaries to `~/.cargo/bin` by default. If your shell cannot find
`contraman` or `getpass`, add that directory to your `PATH`.

For zsh:

```sh
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

For bash:

```sh
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

If you only want a shell alias instead of a second binary, add this to your shell
config:

```sh
alias getpass='contraman get'
```

## Setting Passwords

By default, `set` prompts for the password without echoing it to the terminal:

```sh
contraman set github
```

You can also pass a password directly:

```sh
contraman set github --password "example-password"
```

Passing passwords on the command line can expose them through shell history or
process listings, so the interactive prompt is the safer default.

## Getting Passwords

Fetching an entry prints the password to stdout and also copies it to the system
clipboard:

```sh
contraman get github
```

Clipboard support is implemented with platform commands:

- macOS: `pbcopy`
- Windows: `clip`
- Linux/Unix: `wl-copy`, then `xclip`

If none of those commands are available, `get` fails with an error.

## Entry Index

The OS keychain stores the passwords, but it does not provide a portable way for
this app to enumerate all saved entries. To support `contraman list`, the app
keeps an `entries.txt` file containing only entry names.

The index directory is chosen in this order:

1. `CONTRAMAN_DATA_DIR`, if set.
2. The platform's local data directory, under `contraman`.
3. A `.contraman` directory in the current working directory.

If none of those locations can be created, the command fails and asks you to set
`CONTRAMAN_DATA_DIR` to a writable path.

## Development

This is a single-binary Cargo project:

```sh
cargo check
cargo run -- list
```

Main dependencies:

- `clap` for CLI parsing.
- `keyring` for OS credential storage.
- `rpassword` for hidden password prompts.
- `dirs` for finding platform data directories.
