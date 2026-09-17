# Array sorting in the self-hosted VM

`array_sort(values)` is recognized by the shared VM builtin registry and
forwarded by the PBLang dispatcher to the existing retained-host implementation.
It requires exactly one array argument. Direct calls, aliases and higher-order
calls work; local/global shadows retain precedence. Optional compiler call
metadata is not required for older chunks.

The result has a new outer array. Sorting does not reorder the input array;
nested maps/arrays remain shared. Number and string ordering, mixed-value
behavior and wrong-type errors are inherited from the retained host. This is
not a new comparator API or a PBLang-native sorting engine. Applications that
need a custom comparator should use the existing library-level sort contracts.

The bounded VM checks the input against its existing value-size policy before
the retained call. This is not hard CPU-time, transitive heap or native-process
isolation: one host sort is still a single VM builtin operation. The ordinary
unbounded execution API receives no new size/time limit.

The sort branch is appended after existing builtin cases so previously handled
builtins do not evaluate another branch on their common path. The registry
adds one name. No general CPU/RAM reduction for the VM is claimed.

Run `tests/semantics/selfhost_array_sort_builtin_test.sapp` for numeric/string,
empty/mixed collection, alias/shadow, shallow-copy, arity/type, optimized chunk,
legacy metadata and bounded-value regressions. The File Catalog fixture also
checks sorted rows through actual self-hosted module imports.
