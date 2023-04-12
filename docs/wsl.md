# Configurations for WSL

Add one of the following to your shell configuration (for e.g. .bashrc, .zshrc or config.fish). For advanced configurations consult the documentation of your shell.

## Bash/Zsh

### gpg-agent

```bash
export GPG_AGENT_SOCK=$(gpgconf --list-dir agent-socket)
WIN_IP=$(grep /etc/resolv.conf -e nameserver | awk '{print $2}')
GPG_BRIDGE_PORT="4321"

if ss -a | grep -q "$GPG_AGENT_SOCK"; then
    rm -f "$GPG_AGENT_SOCK"
fi
(setsid nohup socat UNIX-LISTEN:"$GPG_AGENT_SOCK,fork" TCP:"$WIN_IP:$GPG_BRIDGE_PORT" >/dev/null 2>&1 &)
```

### ssh-agent

```bash
export SSH_AUTH_SOCK="$(gpgconf --list-dir agent-ssh-socket)"
WIN_IP="$(grep /etc/resolv.conf -e nameserver | awk '{print $2}')"
SSH_BRIDGE_PORT="4322"

if ss -a | grep -q "$SSH_AUTH_SOCK"; then
    rm -f "$SSH_AUTH_SOCK"
fi

(setsid nohup socat UNIX-LISTEN:"$SSH_AUTH_SOCK,fork" TCP:"$WIN_IP:$SSH_BRIDGE_PORT" >/dev/null 2>&1 &)
```

## Fish

### gpg-agent

```fish
set -x GPG_AGENT_SOCK (gpgconf --list-dir agent-socket)
set WIN_IP (grep /etc/resolv.conf -e nameserver | awk '{print $2}')
set GPG_BRIDGE_PORT "4321"

if ss -a | grep -q "$GPG_AGENT_SOCK";
    rm -f "$GPG_AGENT_SOCK"
end
setsid nohup socat UNIX-LISTEN:"$GPG_AGENT_SOCK,fork" TCP:"$WIN_IP:$GPG_BRIDGE_PORT" >/dev/null 2>&1 &
```

### ssh-agent

```fish
set -x SSH_AUTH_SOCK (gpgconf --list-dir agent-ssh-socket)
set WIN_IP (grep /etc/resolv.conf -e nameserver | awk '{print $2}')
set SSH_BRIDGE_PORT "4321"

if ss -a | grep -q "$SSH_AUTH_SOCK";
    rm -f "$SSH_AUTH_SOCK"
end
setsid nohup socat UNIX-LISTEN:"$SSH_AUTH_SOCK,fork" TCP:"$WIN_IP:$SSH_BRIDGE_PORT" >/dev/null 2>&1 &
```
