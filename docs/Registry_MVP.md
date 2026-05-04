# Registry MVP – Σχεδίαση

Τελευταία ενημέρωση: Φεβρουάριος 2025.

## Στόχος

Να οριστεί το **ελάχιστο εφικτό** σύστημα registry ώστε:
- `sapphire install` να μπορεί να τραβάει dependencies από remote (registry ή Git) εκτός από local paths.
- `sapphire publish` να ανεβάζει πακέτο σε registry.
- Το lockfile να επεκταθεί (Lockfile v2) ώστε να pin-άρει name/version/source/checksum χωρίς να σπάσει το υπάρχον contract.

## Τρέχουσα κατάσταση

- **sapphire install / --lock**: δημιουργούν `pblang.lock` από `pblang.toml`· dependencies είναι **path-based** (local ή paths που resolve από manifest dir).
- **Lockfile**: περιέχει manifest_sha256 και για κάθε dependency path + sha256 (content hash). Δες [Lockfile_contract.md](Lockfile_contract.md).
- **Import resolution**: το VM/resolver χρησιμοποιεί lockfile για να βρει paths· cross-machine reproducibility δεν εγγυάται ακόμα (paths είναι absolute/env-dependent).

## Lockfile v2 (προτεινόμενο σχήμα)

Κρατάμε το υπάρχον format και **προσθέτουμε** optional πεδία για registry packages:

Για κάθε dependency στο lockfile:
- **path** (υπάρχον): resolved path για local/file-based.
- **sha256** (υπάρχον): hash περιεχομένου.
- **name** (νέο, optional): package name π.χ. `@sapphire/http`.
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
- **Fetch**: το CLI κατεβάζει το tarball, το ξεπακετάρει σε cache dir (π.χ. `~/.sapphire/cache` ή `./.sapphire/cache`), και ενημερώνει το lockfile με path + sha256 + name + version + source.

## pblang.toml (dependencies)

Επεκτείνεται το `[dependencies]`:

- **Local**: `"mylib" = { path = "./lib" }` (ή απλά path string αν το υπάρχον format το επιτρέπει).
- **Registry**: `"@sapphire/http" = "1.2.0"` ή `"@sapphire/http" = { version = "1.2.0" }`.
- **Git**: `"github.com/user/repo" = { git = "https://github.com/user/repo", rev = "abc123" }`.

Το `sapphire install` (ή `--lock`) κάνει resolve: για registry entries κάνει lookup στο index, fetch tarball, unpack σε cache, και γράφει στο lockfile το resolved path (στο cache) + sha256 + name + version + source.

## sapphire publish (MVP — υλοποιήθηκε)

- **`sapphire publish --registry-dir \<dir\> [path]`**: Διαβάζει `pblang.toml` (απαιτεί [package] name και version). Φτιάχνει tarball του package (ίδιο file set με hash_dir: όλα τα αρχεία εκτός `target/` και dotfiles). Γράφει **\<name\>-\<ver\>.tar.gz** στο \<dir\> (όπου name με `/` αντικαθίσταται από `_`) και ενημερώνει **\<dir\>/index.json** με entry `packages[name][version] = { url: file:///..., sha256 }`. Από εκεί άλλο project μπορεί να κάνει `sapphire install --registry-index file:///path/to/dir/index.json`.
- Δεν υλοποιείται ακόμα: ανέβασμα σε remote (API/auth).

## Σειρά υλοποίησης (προτείνεται)

1. **Lockfile v2 schema**: προσθήκη optional πεδίων (name, version, source, resolved/fetch_url) στο υπάρχον format· backward compatible (αν λείπουν, συμπεριφορά ως σήμερα).
2. **Resolve registry entry**: στο sapphire-pm, όταν ένα dependency είναι registry-style (name + version), κάνε HTTP GET στο index, βρες tarball URL, κατέβασμα, unpack σε cache, υπολογισμός sha256, και εγγραφή στο lockfile με path = cache path.
3. **sapphire install**: επέκταση ώστε να καλεί το (2) για κάθε registry dependency.
4. **sapphire publish**: Υλοποιήθηκε για **local registry dir** (tarball + index.json)· remote upload μελλοντικά.

## Σχετικά

- [Lockfile_contract.md](Lockfile_contract.md) — τρέχον lockfile contract.
- [Future_plans.md](Future_plans.md) — Phase 2 Package manager, Registry MVP.
- [Report.md](Report.md) — τι έχει υλοποιηθεί.
