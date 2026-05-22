# tutorial_data_science — Data Processing Demo

A self-contained data processing pipeline built with pblang, demonstrating
CSV parsing, type conversion, filtering, grouping, aggregation, and
statistical analysis.

## Features Demonstrated

| Feature              | How it's used                                   |
|----------------------|--------------------------------------------------|
| **import**           | `csv.sapp`, `math.sapp`, `fs.sapp`              |
| **functions**        | Pure transformation functions, aggregators       |
| **arrays**           | Row-oriented data records, sorting               |
| **maps**             | Column-keyed data rows, grouping dictionaries    |
| **iteration**        | `for i len(arr)` to process each row             |
| **map/filter/reduce**| Filter by category, map to revenue, reduce sums  |
| **error handling**   | `try/catch` on type conversions                  |
| **file I/O**         | Export results to CSV file                       |

## Setup & Run

```bash
# From the pblang project root:
pblang run examples/tutorial_data_science/main.sapp
```

## Pipeline Steps

1. **Parse CSV** — Parse inline CSV string into array of maps
2. **Type Conversion** — Convert string fields to numbers
3. **Filtering** — Select only "electronics" products
4. **Mapping** — Transform each row into a revenue record
5. **Grouping & Aggregation** — Group by category, compute totals
6. **Overall Statistics** — Grand totals and averages
7. **Top Performers** — Find best product by custom scoring
8. **Export** — Write summary as CSV to `output_summary.csv`

## Sample Output

```
=== Step 1: Parse CSV ===
Parsed 10 rows

First 3 rows:
  {"id":"1","product":"Widget A",...}

=== Step 3: Filtering (electronics only) ===
Electronics products: 5
  Widget A — $29.99 x 150 units
  ...

=== Step 5: Group by category + Aggregate ===
  Category: electronics
    Products:      5
    Total Revenue: $16097.3
    ...

  Category: toys
    Products:      5
    Total Revenue: $17233.75
    ...

=== Step 6: Overall Statistics ===
Total Products Sold: 1690
Grand Revenue:       $33331.05
Average per Product: $3333.105
```
