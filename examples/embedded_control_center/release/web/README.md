# Web release

The hardware-capable Web UI is served dynamically by the authenticated,
loopback-only PBLang bridge. Run it from the PBLang project root:

```sh
pblang run examples/embedded_control_center/web.sapp --allow-all
```

Then open `http://127.0.0.1:8766/` while the foreground bridge is running.
The session credential is generated in memory for each launch and is inserted
only into that served page. A reusable static bundle is intentionally not
published because it could not securely contain this runtime credential and
would not have access to the host serial controller by itself.
