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

---

## Class semantics (v0.1)

- **Class sugar**: Η δήλωση `class Point { ... }` είναι **sugar** πάνω από maps + functions:
  - Η έκφραση `Point(x, y)` δημιουργεί instance map και αντιστοιχεί σε κλήση `Point.new(x, y)` όταν
    υπάρχει `fn new(...)` μέσα στην class.
  - Αν δεν υπάρχει `new`, η γλώσσα μπορεί να προσφέρει auto-constructor που αντιστοιχίζει τα
    ορίσματα positional στα δηλωμένα fields, αλλά αυτό δεν παγιώνεται πλήρως στο v0.1 (implementation detail).
- **self**:
  - Μέσα στις method δηλώσεις (`fn method(self, ...)` ή ισοδύναμο sugar) το **`self`** είναι
    δεσμευμένο σε instance map με όλα τα fields.
  - Η πρόσβαση σε fields γίνεται ως `self.x`, `self.y` και είναι ισοδύναμη με map lookups στο
    underlying map.
- **Methods ως τιμές**:
  - Οι μέθοδοι είναι first-class: μπορούν να ανατεθούν σε μεταβλητές ή να περαστούν ως ορίσματα,
    και να κληθούν ως `instance.method()` ή `let f = instance.method; f()`.

## Match semantics (v0.1)

- **Μη εξαντλητικά match**:
  - Ένα `match` που δεν καλύπτει όλες τις πιθανές τιμές ΔΕΝ εγείρει σφάλμα· αν δεν ταιριάξει
    κανένα arm, το αποτέλεσμα του match είναι **`nil`**.
- **Σειρά arms**:
  - Οι arms ελέγχονται **από πάνω προς τα κάτω** και ο πρώτος arm που ταιριάζει κερδίζει.
  - Δεν υπάρχει fall-through σε επόμενο arm μετά από επιτυχημένο match.
- **Wildcard `_`**:
  - Το `_` ταιριάζει οποιαδήποτε τιμή· συνίσταται να είναι ο τελευταίος arm και λειτουργεί ως
    φιλική default περίπτωση.

## Defer semantics (v0.1)

- **LIFO παραγγελία**:
  - Τα blocks που δηλώνονται με `defer` εκτελούνται σε **LIFO** σειρά (τελευταίο-defer, πρώτο-run)
    όταν το τρέχον scope αποχωρεί.
- **Πότε τρέχει το defer**:
  - Τα deferred blocks εκτελούνται:
    - σε **κανονικό return** από function,
    - σε **early return** από nested if/while/for,
    - σε **error/throw** που εγκαταλείπει το scope,
    - στο φυσικό τέλος του block όταν δεν υπάρχει explicit return.
  - Το defer εκτελείται στο scope όπου δηλώθηκε, ανεξάρτητα από το πού βρίσκεται το σημείο εξόδου.
- **Errors μέσα σε defer**:
  - Σφάλματα που προκύπτουν μέσα σε deferred block **δεν** επαν-ρίχνονται προς τα έξω·
    αγνοούνται σιωπηρά ώστε το defer cleanup να μην κρύβει το αρχικό error path.

---

## Class semantics (v0.2)

- **Class sugar**: Η δήλωση `class Point { ... }` είναι **sugar** πάνω από maps + functions:
  - Η έκφραση `Point(x, y)` δημιουργεί instance map και αντιστοιχεί σε κλήση `Point.new(x, y)` όταν
    υπάρχει `fn new(...)` μέσα στην class.
  - Αν δεν υπάρχει `new`, η γλώσσα προσφέρει **auto-constructor** που αντιστοιχίζει τα
    ορίσματα positional στα δηλωμένα fields (π.χ. `class Point { x, y }` → `Point(3, 4)` δημιουργεί map με `x: 3, y: 4`).
  - Η έκφραση `ClassName(args)` είναι ισοδύναμη με `ClassName.new(args)` όταν υπάρχει `fn new(...)`.
- **self**:
  - Μέσα στις method δηλώσεις (`fn method(self, ...)` ή ισοδύναμο sugar) το **`self`** είναι
    δεσμευμένο σε instance map με όλα τα fields.
  - Η πρόσβαση σε fields γίνεται ως `self.x`, `self.y` και είναι ισοδύναμη με map lookups στο
    underlying map.
  - Το `self` είναι **implicit** στις method calls: `instance.method(args)` → `method(instance, args)`.
- **Methods ως τιμές**:
  - Οι μέθοδοι είναι first-class: μπορούν να ανατεθούν σε μεταβλητές ή να περαστούν ως ορίσματα,
    και να κληθούν ως `instance.method()` ή `let f = instance.method; f()`.
  - Η method dispatch αναζητά το method στο class method table (map που περιέχει όλα τα `fn` της class).
- **Constructor dispatch**:
  - `ClassName(args)` → `ClassName.new(args)` (explicit `fn new`).
  - `ClassName(args)` → auto-constructor (positional field assignment) αν δεν υπάρχει `fn new`.
  - Το `init(self, ...)` είναι ένα ειδικό method που καλείται αυτόματα από το auto-constructor αν υπάρχει.

## Closure capture rules (v0.2)

- **Lexical scoping**: Τα closures κληρονομούν το lexical scope του ορισμού τους.
- **Capture by reference**: Τα closures κάνουν **capture** τις μεταβλητές από το enclosing scope **by reference** (όχι by value).
  - Αν η captured μεταβλητή αλλάξει μετά το closure definition, το closure βλέπει τη νέα τιμή.
  - Παράδειγμα:
    ```sapphire
    let x = 1;
    let f = fn() { return x; };
    x = 2;
    print f();  # prints 2
    ```
- **Mutable capture**: Τα closures μπορούν να τροποποιούν captured μεταβλητές:
  ```sapphire
  let counter = 0;
  let inc = fn() { counter = counter + 1; return counter; };
  print inc();  # 1
  print inc();  # 2
  ```
- **No capture of undeclared variables**: Τα closures δεν μπορούν να κάνουν capture μεταβλητές που δεν έχουν δηλωθεί στο enclosing scope (runtime error).

## Map/filter/reduce contracts (v0.2)

- **`array.map(fn)`**: Εφαρμόζει τη συνάρτηση `fn` σε κάθε στοιχείο του array και επιστρέφει νέο array με τα αποτελέσματα.
  - `fn` λαμβάνει ένα όρισμα: το στοιχείο.
  - Παράδειγμα: `[1, 2, 3].map(fn(x) { return x * 2; })` → `[2, 4, 6]`.
- **`array.filter(fn)`**: Φιλτράρει το array κρατώντας μόνο τα στοιχεία για τα οποία η `fn` επιστρέφει truthy τιμή.
  - `fn` λαμβάνει ένα όρισμα: το στοιχείο.
  - Παράδειγμα: `[1, 2, 3, 4].filter(fn(x) { return x % 2 == 0; })` → `[2, 4]`.
- **`array.reduce(fn, initial)`**: Συγκεντρώνει τα στοιχεία του array σε μία τιμή χρησιμοποιώντας τη συνάρτηση `fn`.
  - `fn` λαμβάνει δύο ορίσματα: accumulator και current element.
  - Το `initial` είναι η αρχική τιμή του accumulator.
  - Παράδειγμα: `[1, 2, 3].reduce(fn(acc, x) { return acc + x; }, 0)` → `6`.
- **Εγγυήσεις**:
  - Τα `map`, `filter`, `reduce` δεν τροποποιούν το αρχικό array (immutable operations).
  - Η σειρά επεξεργασίας είναι από αριστερά προς τα δεξιά (index 0 → length-1).
  - Τα `map` και `filter` επιστρέφουν νέο array (δεν είναι in-place).
  - Το `reduce` επιστρέφει την τελική τιμή του accumulator.

