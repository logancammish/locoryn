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

You can also pin a release with `--tag v1.2.2`. The installer expects release
assets to use the repository's standard names, for example
`locoryn-1.2.2-linux-x86_64.tar.gz`.

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

## Build every desktop target locally

From any Linux host, the local builder can produce Linux x86_64, Linux ARM64,
Windows x86_64, and Windows ARM64 packages in one invocation:

```sh
./linux_installations/build-locally.sh --install-tools
```

The `--install-tools` flag installs `cross` and `cargo-xwin` with Cargo when
they are missing. Later builds can omit it. The host must also have:

- Rust installed through `rustup`;
- a running Docker or Podman engine;
- LLVM/Clang tools (`clang-cl`, `lld-link`, and `llvm-rc`); and
- `zip`, `tar`, and `sha256sum`.

For example, the system packages are commonly named `clang`, `lld`, `llvm`,
`zip`, and either `docker` or `podman`. Package-manager commands differ between
Linux distributions, so the script checks these tools and identifies anything
missing without trying to modify the operating system.

By default, every online CPU core is divided across four concurrent Cargo
builds. Override the total CPU budget with `--cores NUMBER` or the
`LOCORYN_BUILD_CORES` environment variable.

Completed archives, local Linux installers, checksums, and an instruction file
are placed under the conspicuous repository-root directory:

```text
LOCAL-BUILDS/locoryn-VERSION-TIMESTAMP/READY-TO-INSTALL/
```
