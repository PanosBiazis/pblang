# PBLang Trademark & Brand Attribution Policy

**Creator & Brand Steward:** Panagiotis (Panos) Biazis (`@PanosBiazis`)
**Official Repository:** <https://github.com/PanosBiazis/pblang>
**Software License:** Apache License, Version 2.0 (`LICENSE` & `NOTICE`)

---

## 1. Purpose

`PBLang` is open-source software licensed under the **Apache License, Version 2.0**. While Apache-2.0 grants broad copyright and patent permissions to use, modify, and redistribute the software, **Section 6 of the Apache License, Version 2.0** expressly states that the license does *not* grant permission to use the trade names, trademarks, service marks, or product names of the Licensor, except as required for reasonable and customary use in describing the origin of the Work and reproducing the content of the `NOTICE` file.

This policy ensures two balanced goals:
1. **Welcoming Free Community & Commercial Adoption:** Developers, researchers, educators, and companies can freely build software with `PBLang` and truthfully state compatibility or origin without bureaucratic hurdles.
2. **Preventing Impersonation & Plagiarism:** Users downloading `"PBLang"` know with certainty that it originates from **Panagiotis (Panos) Biazis (`@PanosBiazis`)** at <https://github.com/PanosBiazis/pblang>, and third parties cannot strip authorship or pass off incompatible/unauthorized forks as official `PBLang` releases.

---

## 2. Covered Marks & Identifiers

This policy covers the following word marks and project identifiers (collectively, the **"PBLang Marks"**):

- **`PBLang`** and **`PBLang Programming Language`**
- **`PBLang Sovereign Runtime`** and **`PBLang Compiler`**
- Official project logos, release seals (`PBIPSEAL` official engine identity), and namespace designations identifying official releases from `https://github.com/PanosBiazis/pblang`.

---

## 3. Permitted Community & Nominative Uses (No Permission Required)

You are warmly encouraged to use the PBLang Marks **without prior written permission** in any of the following truthful, non-misleading ways:

1. **Unmodified Redistribution:** Distributing verbatim, unmodified copies or packages (e.g., Linux distribution packages, container images, Homebrew/Nix formulas) of official `PBLang` releases, provided `LICENSE` and `NOTICE` are preserved intact and the exact upstream version is identified.
2. **Applications Built WITH PBLang:** Naming, marketing, selling, or open-sourcing your own independent applications, libraries, games, scientific tools, or firmware compiled with `PBLang`, and stating *"Built with PBLang"*, *"Compiled with PBLang v4.0.0"*, or *"Powered by PBLang"*.
3. **Ecosystem Libraries & Tooling:** Publishing third-party libraries, IDE extensions, syntax highlighters, tutorials, courses, benchmarks, or research papers using clear community naming such as *"foo-for-pblang"*, *"PBLang syntax plugin for EditorX"*, or *"Benchmarking Quantum Circuits in PBLang"*, provided it is clear your project is an independent community work and not an official release by Panagiotis (Panos) Biazis.
4. **Academic & Technical Citation:** Citing `PBLang` and its creator **Panagiotis (Panos) Biazis** in scientific papers, documentation, and software bills of materials (see `CITATION.cff`).

---

## 4. Prohibited Uses

Without explicit written permission from **Panagiotis (Panos) Biazis (`@PanosBiazis`)**, you **may NOT**:

1. **Impersonate Official Releases:** Distribute a modified version or fork of the compiler, virtual machine, runtime, or standard library under the unadorned name **`PBLang`** (or as an "official" `PBLang` release) in a manner likely to confuse users about whether the software was published or endorsed by Panagiotis (Panos) Biazis.
2. **Misappropriate Authorship ("White-Label Plagiarism"):** Rebrand `PBLang` or substantial portions of its compiler, runtime, or standard library under another product name while removing, obscuring, or falsifying the `NOTICE` and `COPYRIGHT` attribution to **Panagiotis (Panos) Biazis** (which also violates Apache-2.0 Sections 4(c) and 4(d)).
3. **Register Confusingly Similar Marks or Domains:** Register trademarks, company names, top-level package namespaces, or primary domains incorporating `PBLang` in a way that implies official stewardship or endorsement.
4. **Forge Cryptographic Engine Seals:** Misrepresent custom or altered toolchains as carrying official `PanosBiazis/pblang` release provenance.

---

## 5. Guidelines for Forks & Derivative Language Distributions

Forking and experimentation are foundational to open source and fully permitted under Apache-2.0! If you publish a modified fork or derivative distribution of `PBLang`:

- Choose a **distinctive project name** that does not start with or consist solely of `PBLang`.
- Retain `LICENSE` and `NOTICE` in full compliance with Apache-2.0 Section 4.
- Include a clear statement in your `README` and CLI `--version` output such as:
  > *"Derived from PBLang (https://github.com/PanosBiazis/pblang), originally created by Panagiotis (Panos) Biazis (@PanosBiazis), licensed under Apache-2.0."*
