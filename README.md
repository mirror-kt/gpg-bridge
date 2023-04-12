# gpg-bridge

A bridge connects openssh-portable and GnuPG on Windows.

## What is this?

There may be a need to share gpg-agent with an SSH remote server or WSL from Windows.

GPG has a [feature to share gpg-agent with remote servers](https://wiki.gnupg.org/AgentForwarding), but this feature is [not fully supported on Windows](https://github.com/PowerShell/Win32-OpenSSH/issues/1564).

This project solves the problem by bridging the Unix Domain Socket in gpg-agent to TCP.

## Docs

Please read the documents under [docs](./docs/) directory.
