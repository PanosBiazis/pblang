# Threads – Σχεδίαση και μελλοντική υλοποίηση

Τελευταία ενημέρωση: Φεβρουάριος 2025.

## Στόχος

Να επιτρέπεται η εκτέλεση κώδικα Sapphire σε παράλληλα νήματα (threads) για:
- I/O overlap (π.χ. HTTP requests ενώ τρέχει game loop)
- CPU-bound tasks σε πολυπύρηνα συστήματα
- Async-style concurrency (spawn, join, shared state με προσοχή)

## Τρέχουσα κατάσταση

- Το VM και ο runtime είναι **single-threaded**. Τα builtins (sleep_ms, tcplisten, κ.λπ.) τρέχουν στο ίδιο thread.
- Το Rust runtime χρησιμοποιεί `thread_local!` και `std::thread` εσωτερικά (π.χ. σε tests, LSP), αλλά η γλώσσα Sapphire δεν εκθέτει threads στο script.

## Προτεινόμενο API (μελλοντικά)

### Επιλογή A: Spawn / Join

```sapphire
// spawn(fn) → handle
let h = spawn(fn() {
    sleep_ms(1000);
    return 42;
});
// ... άλλος κώδικας ...
let result = join(h);  // block μέχρι να τελειώσει, επιστρέφει return value ή error
```

### Επιλογή B: Channels (producer/consumer)

```sapphire
let ch = channel();
spawn(fn() {
    ch.send(42);
});
let x = ch.recv();  // block
```

### Επιλογή C: Host-provided workers

Ο embedder παρέχει `spawn`/`join` ως capability (όπως allow_fs). Το VM δεν έχει built-in thread pool, αλλά ο host καλεί Sapphire chunks σε worker threads και συγχρονίζει.

## Τεχνικές προκλήσεις

1. **Shared state**: Arrays/maps είναι reference-counted. Πρόσβαση από πολλά threads χωρίς lock → data races. Πρέπει είτε immutable sharing, είτε lock API, είτε “no shared mutable state” (μόνο message passing).
2. **VM instance**: Κάθε thread μπορεί να έχει δικό του VM (απλό) ή shared VM με lock (πολύπλοκο). Συνήθως 1 VM per thread απλοποιεί.
3. **Builtins**: Τα I/O builtins (fs, net, http) πρέπει να είναι thread-safe ή να απαγορεύεται η κλήση τους από spawned threads εκτός αν ο host το επιτρέπει.
4. **Limits & sandbox**: VmLimits και StdlibConfig ισχύουν ανά VM· αν spawn = νέο VM, το spawn μπορεί να κληρονομήσει limits.

## Μοντέλο: VM-per-thread + message passing

(Απόφαση σχεδίασης σύμφωνα με Help.md / Future_plans.)

- **1 VM ανά thread**: Κάθε spawned task τρέχει σε νέο OS thread με δικό του VM. Δεν μοιράζονται heap/frames/stack.
- **Καμία κοινή mutable state**: Arrays/maps/modules δεν περνούν απευθείας μεταξύ threads (RC θα έφερνε data races). Η επικοινωνία γίνεται μόνο με **message passing**.

### Τι μπορεί να μεταφερθεί (transferable values)

- **Ασφαλή για μεταφορά** (deep copy ή serialize): numbers, bools, strings, nil. Arrays και maps μπορούν να μεταφερθούν με **deep copy** (αντίγραφο δέντρου τιμών χωρίς shared references) ώστε το child VM να δει αντίγραφα, όχι τα ίδια RC αντικείμενα.
- **Μη μεταφερτά απευθείας**: handles (tcp/sqlite/ffi), closures με upvalues σε άλλο VM, module references. Για δεδομένα: serialize (π.χ. json_stringify) και στο child json_parse.

### Σημασιολογία errors

- **spawn(fn)**: Αν η fn πέσει σε runtime error, το parent το βλέπει όταν καλεί **join(handle)**. Το join επιστρέφει είτε την return value της fn είτε error value (π.χ. RuntimeError).
- **channel send/recv**: recv block· αν το channel κλείσει, recv μπορεί να επιστρέφει nil ή "closed" value. Timeouts (αν προστεθούν) via limits ή host.

### Sandbox & limits

- **allow_spawn** (ή ενσωμάτωση στο capabilities): Σε sandbox το spawn απενεργοποιείται (runtime error "spawn not allowed").
- **Κληρονομιά VmLimits**: Το child VM παίρνει αντίγραφο των limits του parent (max_instructions, max_stack_depth, κ.λπ.).

### Builtins σε spawned threads

- Ορίζεται ποια builtins επιτρέπονται σε spawned VMs (π.χ. μόνο pure + channel)· ή απαγόρευση by default. Handles (tcp/sqlite/ffi) δεν μοιράζονται με parent· script κλείνει handles (defer βοηθά).

## Ροードμαπ

- **Σχεδίαση**: Αυτό το έγγραφο.
- **Επόμενο βήμα**: Απόφαση για API (spawn/join vs channels vs host-only).
- **Υλοποίηση**: Νέο builtin `spawn(fn)` που τρέχει τη fn σε νέο OS thread με δικό του VM (ή shared VM με mutex), και `join(handle)` που επιστρέφει την τιμή. Απαιτεί αλλαγές σε vm.rs και σε value passing μεταξύ threads (serialization ή shared heap με locks).
- **Ασφάλεια**: Σε sandbox mode, `spawn` μπορεί να απενεργοποιείται (allow_spawn: bool).

## Σχετικά

- [Language_Semantics.md](Language_Semantics.md) – capabilities, limits
- [Future_plans.md](Future_plans.md) – γενικό roadmap
