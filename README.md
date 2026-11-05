# Jade for Horizon

An unofficial port of [Jade](https://github.com/Snownee/Jade) 26.3.5 by Snownee to
[Horizon](https://github.com/horizon-rust/horizon): the look-at overlay for blocks and entities, its providers, its
settings under Options > Mods, and its keys.

## What differs from Jade

Rewritten in Rust for Horizon's hooks, so no original code runs. Other mods register with this port's own provider
functions, not Jade's Java API. Themes, opacity, text background, profiles, the ignore lists, boss bar overlap,
TTS, perspective mode, reach, icon positions, the `/jade` and `/jadec` commands, the item mod name and the plugin
sub-options are ported. These still differ:

- Themes are drawn with solid fills in the theme's colours, not Jade's sprites.
- The position adjust screen is a setting you cycle through, not a screen you drag on.
- The sniffer's dig cooldown isn't shown (Horizon's client has no such value).
- Energy Storage has nothing to show: Horizon has no shared energy API for other mods.
- Access variant and NPC details are approximate.
- Dedicated servers: Horizon doesn't build native mods for them, so "server" here means a LAN host.

## Install

`jade-26.3.5.hnmod` in this repository is the packaged native mod, checked with `cargo horizon-mod check`. It's built
for the Horizon commit named below and needs no other mod.

## Build

This is a [Horizon](https://github.com/horizon-rust/horizon) native mod bundle: Rust mixins compiled into the game.
It was built and tested on Horizon commit `6475f62` (Minecraft 26.3, native hooks API 359), as `native-6475f62-2fcf330dd44e`.
Native mods are unsandboxed code that runs with your rights, so read what you build.

Put each bundle's `mixins/<id>` folder in one folder, then:

```bash
horizon-launcher native-build "path/to/that/folder" --trust-native-code
```

## Status

An unofficial port, not made or endorsed by the original authors. Please report problems here, not to them.
It was tested on Windows, in single player and on LAN with one guest. Other platforms and dedicated servers were not tested.

## Credits and licence

Jade, by Snownee. This port is an adapted version, shared under the same licence,
[CC BY-NC-SA 4.0](LICENSE): noncommercial use only, and adaptations must keep this licence.
