# Linux Fedora AIPY

This is the single PBLang example for the canonical PBLang-only product in `products/aipy`, replacing the complete mixed-language `fedora/aipy` mini-project:

- `aipy.py`: deterministic bounded data generation;
- `aipy2.py`: loopback-only hardened web application;
- `aipy3.py`: bounded transactional generation instead of an endless insert loop;
- `connect.php`: credential-free SQLite connection;
- `index.php`: parameterized, bounded pagination and escaped output;
- `index.html`, `style.css`, and `table.html`: generated responsive accessible HTML/CSS;
- `start.py`: one owned PBLang process instead of unmanaged child processes;
- `python.txt`: no Python, PHP, MySQL, NumPy, Pandas, Flask, Plotly, or scikit-learn setup is required.

The app binds to `127.0.0.1`, uses a CSP and other security headers, keeps SQL values parameterized, caps generated rows and pagination, escapes table data, and supports compact/mobile widths.
It uses synthetic seeded records and embeds no personal path, username, device identifier, credential, clipboard content, screenshot, telemetry, or real user data.

Run the in-process example and tests:

```sh
./target/release/pblang run examples/linux_fedora_aipy/main.sapp --allow-all
./target/release/pblang run examples/linux_fedora_aipy/test.sapp --allow-all
```
