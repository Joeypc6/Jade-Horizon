# Jade for Horizon

An unofficial port of [Jade](https://github.com/Snownee/Jade) 26.3.5 by Snownee to
[Horizon](https://github.com/horizon-rust/horizon): the look-at overlay for blocks and entities, its providers, its
settings under Options > Mods, and its keys.

## What differs from Jade

Rewritten in Rust for Horizon's hooks, so no original code runs. Other mods register with this port's own provider
functions, not Jade's Java API. Not ported: themes, alpha and text background, profiles, the ignore list, boss bar
overlap, TTS modes, perspective mode, reach distance, display item position, animation, flip main hand, item mod name,
most plugin sub-options (harvest tool, storage styles and amounts), relative coordinates, some block properties
(explosion resistance, jump, speed, ignite, burn), the sniffer's dig cooldown, and accessibility-only providers.

## Build

This is a [Horizon](https://github.com/horizon-rust/horizon) native mod bundle: Rust mixins compiled into the game.
It was built and tested on Horizon commit `c9d474d` (Minecraft 26.3, native hooks API 80), as `native-c9d474d-fafa5c3981b4`.
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
