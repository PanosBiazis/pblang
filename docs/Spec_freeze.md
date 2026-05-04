# Spec freeze (v0.1)

Τελευταία ενημέρωση: Φεβρουάριος 2025.

## Σκοπός

Το παρόν έγγραφο ορίζει τι θεωρείται **παγωμένο** για το core της γλώσσας Sapphire (syntax, semantics, error model, module system) ώστε να υπάρχει σταθερή βάση για conformance tests, tooling και συμβατότητα.

## Ε scope του freeze (v0.1)

- **Syntax**: Εκφράσεις, δηλώσεις (let, print, if/while/for, fn, try/catch, defer), type annotations, imports, polyglot blocks — όπως τεκμηριώνονται στο parser και στο [Language_Semantics.md](Language_Semantics.md).
- **Truthiness**: nil, false, 0.0, "", [], map{} → false· όλα τα άλλα → true.
- **Indexing**: arrays με αριθμητικό index (0-based), maps με string key· missing key → nil.
- **Error model**: RuntimeError value· try/catch· defer LIFO.
- **Function arity**: σταθερό στο definition· λάθος αριθμός ορισμάτων → runtime error "arity mismatch".
- **Module system**: import(path), resolution μέσω host (VmHost::importmodule), cache, cycle detection· exports.
- **String literals**: escape sequences \\ \" \n \t \r όπως στο [Language_Semantics.md](Language_Semantics.md).

## Τι δεν καλύπτεται ακόμα (εκτός freeze)

- Συγκεκριμένα keywords/constructs που είναι σε πειραματικό στάδιο (π.χ. class sugar, match expression) μπορεί να αλλάξουν.
- Stdlib builtins και API τους εξελίσσονται· τα capabilities (allow_fs, allow_net, κ.λπ.) και τα limits (max_instructions, io_timeout_ms, κ.λπ.) θεωρούνται μέρος του embedding contract αλλά οι default τιμές μπορούν να προσαρμοστούν.

## Συμβατότητα και breaking changes

Για την πολιτική breaking changes και versioning δες [Stability_policy.md](Stability_policy.md). Σε v0.x επιτρέπονται breaking changes με τεκμηρίωση και changelog· το spec freeze δηλώνει τι είναι η **αναφορά** για tests και conformance, όχι απαγορευτικό για εξέλιξη.

## Σχετικά

- [Language_Semantics.md](Language_Semantics.md) — λεπτομέρειες semantics, capabilities, limits.
- [Stability_policy.md](Stability_policy.md) — versioning, breaking changes.
- [Future_plans.md](Future_plans.md) — roadmap και επόμενα βήματα.
