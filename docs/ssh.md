# Configurations for SSH Remote Server

Make sure you have setup gpg agent forward following [the guide](https://wiki.gnupg.org/AgentForwarding).

Directly using socket provided by GnuPG won't work on Windows, so change local socket to a TCP port instead.

```txt
RemoteForward <socket_on_remote_box> 127.0.0.1:4321
```
