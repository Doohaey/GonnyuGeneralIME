# Mobile Rime host validation

This directory holds the deterministic host-side pieces for the pinned mobile Rime toolchain.

## Pinned sources

`engine-lock.json` fixes:

- `librime` to tag `1.17.0` at commit `33e78140250125871856cdc5b42ddc6a5fcd3cd4`
- `librime-lua` to commit `ad1e4a6c98abf634dd34242a747f9b1d5d069fbe`
- `librime-lua` bundled Lua third-party sources to commit `9e5bb71db1544913f8005dadc3df8439c00d08b7`

Fetch the exact source tree into `build/rime-mobile/sources/`:

```bash
python3 platforms/rime/mobile/fetch_sources.py
```

The fetcher ignores the machine-global Git URL rewrite config and always verifies the checked-out full SHA-1.

## Host build entrypoint

For remote or CI validation, build the pinned host toolchain with:

```bash
bash platforms/rime/mobile/build_host.sh
```

The script:

1. fetches pinned sources if they are not already present
2. builds upstream third-party dependencies into an isolated prefix
3. configures librime with merged `librime-lua`
4. installs the host build into `build/rime-mobile/host/prefix/`
5. regenerates Gonnyu Rime resources with `platforms/rime/build.py`
6. writes `build/rime-mobile/host/build-summary.json`

The YAML resources are the source layer. Before an API or app run, deploy them
with the generated `rime_deployer` so Rime creates the `.table.bin`,
`.prism.bin`, and `.reverse.bin` files in the user build directory. The
deployed tree is a validation/build intermediate and is not a netdisk package.

The minimal host behavior check is:

```bash
build/rime-mobile/host/build/bin/rime_deployer --build \
  "$USER_DIR" "$SHARED_DIR" "$USER_DIR/build"
```

Then create a session with the installed API, select `gannyu_fenni` and
`gannyu_lancong`, and simulate `nhk`. Both schemas must return `银行卡` among
the candidates. Candidate order beyond required behavior is not part of the
assertion.

The generated Rime resources now include `resource-manifest.json` with deterministic file sizes and SHA-256 hashes.

## Remote trigger

The repository workflow `.github/workflows/rime-mobile-host.yml` runs the pinned host build on push and manual dispatch. Use that workflow for actual host builds instead of relying on a local signed mobile build.
