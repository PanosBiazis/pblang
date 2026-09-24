# Form workbench

A synthetic Delphi quotation form customized and composed with a second PBLang
form. The original DFM string is unchanged; calculation and review actions are
explicit PBLang functions. No source from the private `sw` tree is copied.

Run from the PBLang repository:

```sh
pblang run examples/form_workbench/main.sapp --allow-all
pblang run examples/form_workbench/main.sapp --allow-all -- --serve
```

Open `http://127.0.0.1:8766`. Enter 7 and 23, press Calculate, and expect 161.
A comment is displayed as escaped text and is not saved. Ctrl+C stops the preview.
The first command checks the example and exits without opening a listener.

Crawler discouragement/throttling and internal-name minimization are ON by default,
independently configurable. Neither guarantees prevention of scraping or reverse
engineering. The example has no accounts and must stay local with synthetic data.
See [the contract and limitations](../../docs/FORM_WORKBENCH.md).
