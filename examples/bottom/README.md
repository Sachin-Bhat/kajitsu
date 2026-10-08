Enable **Settings → Integrations → bottom**, then launch:

```sh
btm --config_location "${XDG_STATE_HOME:-$HOME/.local/state}/kajitsu/bottom.toml"
```

Kajitsu reads `${XDG_CONFIG_HOME:-$HOME/.config}/bottom/bottom.toml` and writes a copy with its `[styles]` replaced. Flags, layout, filters, and non-style comments remain. Missing source configuration uses defaults; malformed TOML leaves the last valid generated file intact and reports an error in the shell log. Source edits trigger regeneration even if the palette stays the same.

Colors apply on the next launch. Avoid `--theme`, which overrides custom config styling. bottom inherits the font from WezTerm. Remove `--config_location` to return to your original configuration; disabling the integration stops exports.
