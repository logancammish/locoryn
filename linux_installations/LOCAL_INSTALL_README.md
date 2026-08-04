# Install this Locoryn build on Linux

This bundle installs the included build for the current user. It does not use
`sudo`, download another release, or change system-wide files.

From this extracted folder, run:

```sh
sh install-linux.sh
```

The application is installed under `~/.local/lib/locoryn`, with a launcher in
`~/.local/bin` and a desktop-menu entry in `~/.local/share/applications`.
Chats and settings remain in your user data directory when you uninstall.

To remove the application later, run:

```sh
~/.local/lib/locoryn/uninstall-linux.sh
```

