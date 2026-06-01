# Registry MVP – Σχεδίαση

Τελευταία ενημέρωση: Μάρτιος 2026.

## Στόχος

Να οριστεί το **ελάχιστο εφικτό** σύστημα registry ώστε:
- `pblang install` να μπορεί να τραβάει dependencies από remote (registry ή Git) εκτός από local paths.
- `pblang publish` να ανεβάζει πακέτο σε registry.
- Το lockfile να επεκταθεί (Lockfile v2) ώστε να pin-άρει name/version/source/checksum χωρίς να σπάσει το υπάρχον contract.

## Τρέχουσα κατάσταση

- **pblang install / --lock**: δημιουργούν `pblang.lock` από `pblang.toml`· dependencies είναι **path-based** (local ή paths που resolve από manifest dir).
- **Lockfile**: περιέχει manifest_sha256 και για κάθε dependency path + sha256 (content hash). Δες [Lockfile_contract.md](Lockfile_contract.md).
- **Import resolution**: το VM/resolver χρησιμοποιεί lockfile για να βρει paths· cross-machine reproducibility δεν εγγυάται ακόμα (paths είναι absolute/env-dependent).

## Lockfile v2 (προτεινόμενο σχήμα)

Κρατάμε το υπάρχον format και **προσθέτουμε** optional πεδία για registry packages:

Για κάθε dependency στο lockfile:
- **path** (υπάρχον): resolved path για local/file-based.
- **sha256** (υπάρχον): hash περιεχομένου.
- **name** (νέο, optional): package name π.χ. `@pblang/http`.
- **version** (νέο, optional): semantic version π.χ. `1.2.0`.
- **source** (νέο, optional): `registry`, `git`, `path`.
- **resolved** (νέο, optional): URL ή path από όπου fetched (registry URL, git URL, ή local path).
- **fetch_url** (νέο, optional): για registry, το URL του tarball.

Η **αλήθεια** παραμένει το content hash (sha256): ίδιο hash = ίδιο περιεχόμενο, ανεξάρτητα από πού ήρθε.

## Registry index (MVP)

- **Απλό index**: στατικό JSON (ή TOML) που περιέχει για κάθε package name λίστα versions με:
  - version
  - tarball URL (ή checksum + URL)
  - optional sha256
- **Πηγή**: αρχικά GitHub Releases ή απλό HTTP server που σερβίρει index + tarballs.
- **Fetch**: το CLI κατεβάζει το tarball, το ξεπακετάρει σε cache dir (π.χ. `~/.pblang/cache` ή `./.pblang/cache`), και ενημερώνει το lockfile με path + sha256 + name + version + source. Για static registries, το `url` μπορεί πλέον να είναι είτε absolute URL είτε path relative προς το `index.json`.

## pblang.toml (dependencies)

Επεκτείνεται το `[dependencies]`:

- **Local**: `"mylib" = { path = "./lib" }` (ή απλά path string αν το υπάρχον format το επιτρέπει).
- **Registry**: `"@pblang/http" = "1.2.0"` ή `"@pblang/http" = { version = "1.2.0" }`.
- **Git**: `"github.com/user/repo" = { git = "https://github.com/user/repo", rev = "abc123" }`.
- **Package entrypoint**: στο `[package]` μπορεί προαιρετικά να υπάρχει **`entry = "src/lib.sapp"`** ώστε τα bare imports (`import("@scope/pkg")`, `import("depname")`) να μη βασίζονται μόνο στο root heuristic `lib/main.*`. Προαιρετικά **`description = "..."`** για packaging/shipping UX: εμφανίζεται στην έξοδο επιτυχίας του `pack`/`bundle` μαζί με name@version.

Το `pblang install` (ή `--lock`) κάνει resolve: για registry entries κάνει lookup στο index, fetch tarball, unpack σε cache, και γράφει στο lockfile το resolved path (στο cache) + sha256 + name + version + source. Το unpack γίνεται πλέον με safe-entry policy: archive paths πρέπει να είναι relative μέσα στο destination και symlink/hardlink entries απορρίπτονται.

Registry/git packages δεν θεωρούνται installable μόνο επειδή unpacked/cloned successfully: αν ένα third-party package δηλώνει `[package].entry`, αυτό πρέπει να είναι valid file μέσα στο package root· αν δεν δηλώνει explicit entry, το package πρέπει παρ' όλα αυτά να περιέχει τουλάχιστον ένα `.sap`/`.sapp` module. Έτσι broken packages απορρίπτονται στο install-time αντί να αφήνουν misleading lock entries.

Αν το cached absolute path ενός registry/git dependency λείπει αλλά το `pblang.lock` έχει αρκετά metadata (`source`, `resolved`, `fetch_url`, `archive_sha256`), ο resolver μπορεί να ξαναστήσει το dependency root από το lockfile αντί να απαιτεί νέο install μόνο και μόνο επειδή χάθηκε το cache path.

Για bare package imports, ο resolver πρώτα κοιτά το `[package].entry` του εγκατεστημένου package manifest και μόνο αν λείπει κάνει fallback στα `lib.sapp`, `lib.sap`, `main.sapp`, `main.sap`. Το `entry` πρέπει να είναι relative path μέσα στο package root.

Για package subpath imports (`import("@scope/pkg:src/foo.sapp")`, `import("depname:src/foo.sapp")`), το subpath επίσης πρέπει να είναι relative, να μένει μέσα στο package root, και να δείχνει σε αρχείο module. Absolute paths, `..` escapes, ή directory targets απορρίπτονται αντί να κάνουν resolve έξω από το installed package ή σε non-file target.

## pblang publish (MVP — υλοποιήθηκε)

- **`pblang publish --registry-dir \<dir\> [path]`**: Διαβάζει `pblang.toml` (απαιτεί [package] name και version). Αν υπάρχει δηλωμένο `[package].entry`, το validate-άρει πριν το publish: πρέπει να είναι relative path μέσα στο package root και να δείχνει σε υπαρκτό αρχείο, όχι directory. Στο τρέχον local-registry MVP, το package πρέπει επίσης να είναι **self-contained**: αν έχει entries στο `[dependencies]`, το publish αποτυγχάνει αντί να παράγει artifact που εξαρτάται από τοπικά ή μη-πακεταρισμένα dependencies. Αν δεν υπάρχει explicit `[package].entry`, το package παραμένει publishable μόνο αν έχει τουλάχιστον ένα pblang module αρχείο: είτε default bare-import entry (`lib.sapp`, `lib.sap`, `main.sapp`, `main.sap`) είτε άλλο `.sap`/`.sapp` module που θα φορτωθεί μέσω package subpath import. Έπειτα φτιάχνει tarball του package (ίδιο file set με hash_dir: όλα τα αρχεία εκτός `target/` και dotfiles). Γράφει **\<name\>-\<ver\>.tar.gz** στο \<dir\> (όπου name με `/` αντικαθίσταται από `_`) και ενημερώνει **\<dir\>/index.json** με entry `packages[name][version] = { url: file:///..., sha256 }`. Από εκεί άλλο project μπορεί να κάνει `pblang install --registry-index file:///path/to/dir/index.json`.
- **Immutability contract**: Το local registry θεωρεί κάθε `name + version` immutable. Αν γίνει δεύτερο publish του ίδιου version με **ίδιο** tarball content, η εντολή επιτρέπεται ως idempotent success. Αν το περιεχόμενο διαφέρει, το publish αποτυγχάνει αντί να αντικαταστήσει σιωπηρά το ήδη δημοσιευμένο package.
- Δεν υλοποιείται ακόμα: ανέβασμα σε remote (API/auth).

## Σειρά υλοποίησης (προτείνεται)

1. **Lockfile v2 schema**: προσθήκη optional πεδίων (name, version, source, resolved/fetch_url) στο υπάρχον format· backward compatible (αν λείπουν, συμπεριφορά ως σήμερα).
2. **Resolve registry entry**: στο pblang-pm, όταν ένα dependency είναι registry-style (name + version), κάνε HTTP GET στο index, βρες tarball URL, κατέβασμα, unpack σε cache, υπολογισμός sha256, και εγγραφή στο lockfile με path = cache path.
3. **pblang install**: επέκταση ώστε να καλεί το (2) για κάθε registry dependency.
4. **pblang publish**: Υλοποιήθηκε για **local registry dir** (tarball + index.json)· remote upload μελλοντικά.

## Επόμενα βήματα (next phase) — Export maps / package surface contracts

Στόχος: να οριστεί τι "εκτίθεται" από ένα package προς τους consumers, πέρα από το τρέχον `[package].entry` και τα subpath imports, ώστε να υπάρχει σαφές contract για το registry architecture.

### Τρέχον baseline

- **Bare import** (`import("@scope/pkg")`): φορτώνει το canonical entry — είτε `[package].entry` είτε fallback σε `lib.sapp` / `lib.sap` / `main.sapp` / `main.sap`.
- **Subpath import** (`import("@scope/pkg:src/foo.sapp")`): το subpath πρέπει να είναι relative, μέσα στο package root, και να δείχνει σε **αρχείο** (όχι directory). Δεν υπάρχει ρητή whitelist: οποιοδήποτε τέτοιο path επιτρέπεται αν το αρχείο υπάρχει.

### Τι σημαίνει "export map" για pblang

- **Export map**: προαιρετικό manifest field (π.χ. `[package].exports` ή `exports` σε ξεχωριστό schema) που δηλώνει **ποια module paths είναι public API** του package. Τα υπόλοιπα αρχεία θεωρούνται internal και δεν θα έπρεπε να βασίζονται σε αυτά εξωτερικοί consumers (ενδεχομένως να απορρίπτονται σε strict mode).
- **Package surface contract**: η συμφωνία ότι το registry (ή το tooling) μπορεί να validate ότι τα imports ενός consumer απευθύνονται μόνο σε exported paths· και ότι το publish μπορεί να ελέγχει ότι τα exported paths υπάρχουν και είναι αρχεία.

### Σύνδεση με τρέχον συμπεριφορά

- Αν **δεν** υπάρχει export map: η συμπεριφορά παραμένει "όλα τα relative subpaths μέσα στο root που είναι αρχεία" (current behavior). Backward compatible.
- Αν **υπάρχει** export map: το resolver (install/import) μπορεί να περιορίζει τα επιτρεπτά subpaths στα ρητά exported entries. Π.χ. `exports = ["src/lib.sapp", "src/extra.sapp"]` → μόνο `import("pkg:src/lib.sapp")` και `import("pkg:src/extra.sapp")` επιτρέπονται· `import("pkg:src/internal.sapp")` → error σε strict mode.

### Προτεινόμενο ελάχιστο schema (για μελλοντική υλοποίηση)

- Στο `[package]`: προαιρετικό **`exports`** = array of strings (relative paths μέσα στο package root, πρέπει να είναι αρχεία). Αν απουσιάζει, συμπεριφορά ως σήμερα (implicit όλα τα valid subpaths).
- Publish-time: αν υπάρχει `exports`, validate ότι κάθε entry είναι relative, μέσα στο root, και δείχνει σε υπαρκτό αρχείο (ίδιο contract με το τρέχον `entry`).
- Import-time (optional strict): αν το package δηλώνει `exports` και ο consumer κάνει subpath import που δεν ανήκει στο exports, το tooling μπορεί να εκδώσει warning ή error (policy choice).

### Μη στόχοι σε αυτό το design note

- Δεν ορίζεται ακόμα conditional exports (π.χ. per-environment)· δεν αλλάζει το lockfile schema σε αυτό το βήμα· δεν υλοποιείται κώδικας — μόνο σχεδίαση για το επόμενο registry-architecture gap.

## Package signing / verification (design note — E3 shipping)

Για πιο ασφαλή distribution και trusted supply chain, μελλοντικά μπορεί να προστεθεί:

- **Tarball / artifact signing**: υπογραφή του `.tar.gz` (π.χ. με Ed25519 ή GPG) και αποθήκευση της signature στο index ή ως ξεχωριστό αρχείο· το `pblang install` να επαληθεύει (optional ή strict mode) πριν unpack.
- **Checksum chain**: το τρέχον **sha256** στο lockfile και στο index είναι baseline· επέκταση με **archive_sha256** vs **content sha256** ή πολλαπλά hashes (SHA-256, SHA-512) για stronger integrity.
- **Trust policy**: allowlist of signing keys per registry ή per package· default-deny για unsigned packages όταν ενεργό policy.

Δεν υλοποιείται σε αυτό το doc — μόνο σημείωση για επόμενο E3/shipping hardening.

## Σχετικά

- [Lockfile_contract.md](Lockfile_contract.md) — τρέχον lockfile contract.
- [Future_plans.md](Future_plans.md) — Phase 2 Package manager, Registry MVP.
- [Report.md](Report.md) — τι έχει υλοποιηθεί.
