# libchar_scale

A plugin that changes the size of any fighter, per costume slot, using a `config_scale.toml` file inside your mod folders. (tested on 13.0.4 and 13.0.5)

![preview](https://github.com/Max904/libchar_scale/blob/main/preview.png?raw=true)

**Requires:** [Skyline](https://github.com/skyline-dev/skyline) and [ARCropolis](https://github.com/Raytwo/ARCropolis).

## Usage

Put a `config_scale.toml` in the root of an enabled mod folder (`sd:/ultimate/mods/<Your Mod>/`), then restart the game:

```toml
kind  = "mario"
slots = [0,1,2]
scale_in_results = false

[[scale]]
value = 1.2
```

This makes Mario costumes 0, 1 and 2 twenty percent bigger.

### Top-level defaults (optional)

| Key | Meaning |
|---|---|
| `kind` | Default fighter for entries that don't set their own. |
| `slots` | Default costume numbers. `-1` means all costumes. |
| `scale_in_results` | Default for entries: `false` = normal size on the results screen. Default `true`. |

### Entries

| Key | Required | Meaning |
|---|---|---|
| `value` | yes | Size multiplier: `1.0` = normal, `1.2` = 20% bigger, `0.8` = smaller. |
| `kind` | no | One fighter name. |
| `kinds` | no | A list of fighters sharing the same size and slots, e.g. `["mario", "luigi"]`. |
| `slots` | no | Costume numbers affected. `-1` = all. |
| `scale_in_results` | no | `false` = this entry doesn't apply on the results screen. Overrides the top-level default. |

An entry needs a fighter and at least one slot, either its own or from the top-level defaults. If entries overlap, the last one wins.

## Notes

- `value` multiplies the size the game uses, so effects that change size (Super/Poison Mushroom, etc.) still work on top of it.
- With `scale_in_results = false`, the fighter returns to normal size on the results screen.
- Fighter names are the internal ones (`mario`, `pikachu`, `ptrainer`, ...), case-insensitive.

## Build

Requires [cargo-skyline](https://github.com/jam1garner/cargo-skyline):

```
cargo skyline build --release
```

The result is `target/aarch64-skyline-switch/release/libchar_scale.nro`.

## Credits

Config scanning modeled on [lib_paramconfig](https://github.com/CSharpM7/lib_paramconfig) by CSharpM7.
