# Vehicle Emissions Lab

This PBLang-only example migrates an earlier Python fuel-consumption analysis
script into a bounded, deterministic and privacy-aware data workflow. It has no
Pandas, NumPy, Matplotlib, Seaborn or scikit-learn runtime dependency.

```text
pblang run examples/vehicle_emissions_lab/main.sapp
pblang run examples/vehicle_emissions_lab/benchmark.sapp
```

The example parses CSV through `stdlib:dataframe.sapp`, validates its shape,
reports missing values and numeric summaries, measures Pearson correlations,
builds category encodings without publishing category labels, selects the
strongest numeric predictor, performs a deterministic train/test split, fits a
one-dimensional ordinary least-squares model and reports MSE, RMSE and R2.

The original script calculated one-hot data for its heatmap but trained the
regression against the original mixed-type dataframe. This edition does not
pretend that categorical features entered the model. Its model scope is
explicitly `best-single-numeric-feature`; categorical encoding remains a
separate exploratory result.

Raw rows and category labels are omitted by default. Plot points are available
only through `include_plot_points: true` and are capped. The included dataset is
synthetic and contains no personal path, identity, credential or downloaded
content. The same UI tree compiles to compact Web, native desktop and Android
records, and the chart is represented by portable visual commands.

Input is capped at 512 Ki characters, 4,096 rows, 64 columns and 32 declared
features. This is an educational analysis pipeline, not a regulatory vehicle
emissions model. It does not implement multivariate regression, causal
inference, categorical coefficients, automatic delimiter detection, arbitrary
file access or a live target renderer.

The original source is represented only by SHA-256
`7b12ac3eeca65e42a0d81f953ec6bb7af9a78ee7f9c26cc4d4a10b8be8f5ba60`.
