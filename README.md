# Jade for Horizon

An unofficial port of [Jade](https://github.com/Snownee/Jade) 26.3.5 by Snownee to
[Horizon](https://github.com/horizon-rust/horizon): the look-at overlay for blocks and entities, its providers, its
settings under Options > Mods, and its keys.

## What differs from Jade

Rewritten in Rust for Horizon's hooks, so no original code runs. Its overlay, providers, themes, config screens, drag-to-place
screen, profiles, `/jade` and `/jadec` commands and the `jade:max_position_deviation` game rule are ported.

### Missing or not the same

- **Jade's Java API.** Java plugins for Jade can't run. Other mods register with this port's own provider functions.
- **Energy icon.** Jade's energy sprite isn't in Horizon's atlas, so the icon style of the energy line shows redstone dust.
- **The game rule** works through mixins on tables compiled into the game, so it is the part most likely to break on a
  Horizon update.

### Not tested

Jade's mouse binds, such as a mouse button for Show Overlay.

## Install

`jade-26.3.7.hnmod` in this repository is the packaged native mod, checked with `cargo horizon-mod check`. It's built
for the Horizon commit named below and needs no other mod.

## Build

This is a [Horizon](https://github.com/horizon-rust/horizon) native mod bundle: Rust mixins compiled into the game.
It was last tested on Horizon commit `6560641` (Minecraft 26.3, native hooks API 411), with AE2, JEI, Elevator, Iron Furnaces and Zoom.
Native mods are unsandboxed code that runs with your rights, so read what you build.

Put each bundle's `mixins/<id>` folder in one folder, then:

```bash
horizon-launcher native-build "path/to/that/folder" --trust-native-code
```

## Status

An unofficial port, not made or endorsed by the original authors. Please report problems here, not to them.
It was tested on Windows, in single player and on LAN with one guest; this version not on a dedicated server. Other platforms were not tested.
A Horizon update can break it until it is rebuilt: it is built for the one Horizon commit named under Build.

## Credits and licence

Jade, by Snownee. This port is an adapted version, shared under the same licence,
[CC BY-NC-SA 4.0](LICENSE): noncommercial use only, and adaptations must keep this licence.
