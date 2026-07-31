# Locoryn Linux installation

The installer downloads a published Locoryn Linux archive and performs a
per-user installation. It does not use `sudo`.

From the repository root:

```sh
./install-linux.sh
```

The interactive flow detects the machine architecture and lets you choose
`x86_64` or `arm64` if that detection is wrong. The `main` channel selects
GitHub's latest stable release; `beta` selects the newest prerelease.

For an unattended installation:

```sh
./install-linux.sh --arch x86_64 --channel main --yes
./install-linux.sh --arch arm64 --channel beta --yes
```

You can also pin a release with `--tag v1.0.1`. The installer expects release
assets to use the repository's standard names, for example
`locoryn-1.0.1-linux-x86_64.tar.gz`.

Files are installed to these per-user locations:

- application: `~/.local/lib/locoryn`
- command: `~/.local/bin/locoryn`
- desktop entry: `$XDG_DATA_HOME/applications/io.github.logancammish.locoryn.desktop`
- icon: `$XDG_DATA_HOME/icons/hicolor/512x512/apps/io.github.logancammish.locoryn.png`

`XDG_DATA_HOME` defaults to `~/.local/share`. Override the application or
command locations with `LOCORYN_INSTALL_DIR` and `LOCORYN_BIN_DIR`.

To uninstall the application while retaining chats and settings:

```sh
./linux_installations/uninstall-linux.sh
```
