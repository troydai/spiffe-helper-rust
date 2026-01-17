# spiffe-helper Examples

This directory contains example configurations for `spiffe-helper`.

## Simple Configuration

The [simple.conf](./simple.conf) example shows the most basic usage of `spiffe-helper`.

```hcl
agent_address = "unix:///run/spire/sockets/agent.sock"
cert_dir = "./certs"
```

In this mode:
1. `spiffe-helper` fetches the SVID and trust bundle from the SPIRE agent.
2. It writes them to the specified `cert_dir`.
3. It keeps them renewed automatically.
4. No signals are sent, and no child processes are managed.

## Managed Process

The [managed_process.conf](./managed_process.conf) example shows how to use `spiffe-helper` to manage a child process (like Nginx).

```hcl
cmd = "/usr/sbin/nginx"
cmd_args = "-g 'daemon off;'"
renew_signal = "SIGHUP"
```

In this mode:
1. `spiffe-helper` fetches the initial certificates.
2. It starts the managed process.
3. Whenever certificates are updated by SPIRE, it sends the `SIGHUP` signal to the Nginx process to trigger a reload.

## PID File Signaling

The [pid_file.conf](./pid_file.conf) example shows how to signal an external process that is NOT managed by the helper.

```hcl
pid_file_name = "/run/my-app.pid"
renew_signal = "SIGUSR1"
```

In this mode:
1. `spiffe-helper` fetches and writes certificates.
2. On every update, it reads the PID from `/run/my-app.pid`.
3. It sends `SIGUSR1` to that PID.

## Running the helper

You can run the helper with these configurations using:

```bash
spiffe-helper --config examples/managed_process.conf
```
