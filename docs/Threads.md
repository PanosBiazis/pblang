# Threads – Σχεδίαση και μελλοντική υλοποίηση

Τελευταία ενημέρωση: Μάρτιος 2026.

## Στόχος

Να επιτρέπεται η εκτέλεση κώδικα Sapphire σε παράλληλα νήματα (threads) για:
- I/O overlap (π.χ. HTTP requests ενώ τρέχει game loop)
- CPU-bound tasks σε πολυπύρηνα συστήματα
- Async-style concurrency (spawn, join, shared state με προσοχή)

## Τρέχουσα κατάσταση

- Το Sapphire εκθέτει πλέον **`spawn(fn)`**, **`join(handle)`**, **`channel()`**, **`send(sender, value)`**, **`recv(receiver)`**.
- Το runtime παραμένει **VM-per-thread**: κάθε spawned task τρέχει σε νέο OS thread με δικό του VM.
- Τα values μεταφέρονται με serialize/deep-copy semantics. Δεν υπάρχει shared RC heap ανάμεσα σε parent/child VMs.
- Τα channel endpoints είναι ξεχωριστά **sender/receiver** values και χρησιμοποιούνται μαζί με τα builtins `send` / `recv`.
- Το τρέχον channel MVP είναι **single-send per sender endpoint**: το `send(sender, value)` καταναλώνει το sender handle και έτσι το `recv(receiver)` μπορεί να δει σωστά closed-channel κατάσταση ως `nil`.

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
{
    let tx = ch.sender;
    let rx = ch.receiver;
    let h = spawn(fn() {
        send(tx, 42);
        return nil;
    });
    join(h);
    let x = recv(rx);  // block
}
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

- **Ασφαλή για μεταφορά** (deep copy ή serialize): numbers, bools, strings, nil. Arrays και maps μεταφέρονται ως αντίγραφα ώστε το child VM να δει δικά του δεδομένα, όχι shared RC references. Τα thread channel endpoint values μεταφέρονται επίσης με ασφαλές tagged representation.
- **Μη μεταφερτά απευθείας**: handles (tcp/sqlite/ffi), module references, builtins/functions ως values που εξαρτώνται από runtime identity άλλου VM. Για δεδομένα: serialize (π.χ. json_stringify) και στο child json_parse.

### Σημασιολογία errors

- **spawn(fn)**: Αν η fn πέσει σε runtime error, το parent το βλέπει όταν καλεί **join(handle)**. Το join επιστρέφει είτε την return value της fn είτε error value (π.χ. RuntimeError).
- **channel send/recv**: `send(sender, value)` καταναλώνει το sender endpoint του MVP. Το `recv(receiver)` block-άρει. Αν το channel κλείσει και δεν υπάρχουν άλλα μηνύματα, επιστρέφει `nil`.

### Sandbox & limits

- **allow_spawn** (ή ενσωμάτωση στο capabilities): Σε sandbox το spawn απενεργοποιείται (runtime error "spawn not allowed").
- **Κληρονομιά VmLimits**: Το child VM παίρνει αντίγραφο των limits του parent (max_instructions, max_stack_depth, κ.λπ.).

### Builtins σε spawned threads

- Ορίζεται ποια builtins επιτρέπονται σε spawned VMs (π.χ. μόνο pure + channel)· ή απαγόρευση by default. Handles (tcp/sqlite/ffi) δεν μοιράζονται με parent· script κλείνει handles (defer βοηθά).

## Ροードμαπ

- **Done**: `spawn/join` MVP.
- **Done**: `channel()` + `send(sender, value)` + `recv(receiver)` MVP.
- **Done**: transferable capture path για spawned closures, μόνο για safe serialized values.
- **Done**: regression tests για value return, error propagation, inherited limits, closed-channel behavior, deterministic send/recv, non-transferable rejection.
- **Επόμενο βήμα**: richer channel ergonomics (`close`, multiple sends per endpoint, πιθανό `select`) μόνο αν χρειαστεί, χωρίς shared heap model.

## Σχετικά

- [Language_Semantics.md](Language_Semantics.md) – capabilities, limits
- [Future_plans.md](Future_plans.md) – γενικό roadmap
