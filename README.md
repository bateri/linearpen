# linearpen

Your Linear issues in a terminal pane, and an agent started on the ones you pick. It only reads:
nothing is changed in Linear.

linearpen is a pen for bateri: in a terminal that shows pencere
views it opens in the pane, and anywhere else it prints your issues as text.

## Try it

```sh
cargo build --release
LINEAR_API_KEY=<a personal API key with Read permission> target/release/linearpen --text
```

Make the key in Linear under Settings, Account, Security. The key is also read from the file `key`
in `~/.config/linearpen` (or `$XDG_CONFIG_HOME/linearpen`); keep it readable by you alone.

linearpen talks to Linear with `curl`, so `curl` has to be installed. The key goes to `curl` on its
standard input, not as an argument and not in its environment.

## License

GPL-3.0-or-later.
