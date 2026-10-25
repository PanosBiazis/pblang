# Application and disk idea intake — 2026-09-16

## Coverage

Six connected physical disks have mounted data filesystems: one internal and
five external. A bounded metadata traversal visited 19637 entries and counted
15939 files across all six. This is partial, not a full read of all disks.
Each volume used at most 4,000 entries, depth four and a three-second traversal
budget; symlinks and other mounted filesystems were not followed. Virtual/system,
credential and dependency directories were excluded. Unmounted partitions were
not mounted or read. Individual filenames and personal directory paths were not
retained in the report. Limits belong to this discovery pass, not compilation.

| Volume | Entries | Files | Directory entries | Partial |
| --- | --- | --- | --- | --- |
| internal | 4000 | 2727 | 532 | True |
| external-1 | 4000 | 3556 | 444 | True |
| external-2 | 4000 | 3147 | 853 | True |
| external-3 | 2472 | 2260 | 212 | True |
| external-4 | 4000 | 3168 | 832 | True |
| external-5 | 1165 | 1081 | 84 | True |

Read category metadata from 239 desktop entry files,
without launching applications or reading their documents/configuration.
Category memberships overlap: {"AudioVideo": 5, "Development": 21, "Education": 2, "Game": 7, "Graphics": 9, "Network": 29, "Office": 8, "Science": 1, "System": 35, "Utility": 32}.
These are desktop entries, not a count of distinct installed applications.

Refreshed 253 cached manifests (219 Codex, 34 Claude) and their name/description
keywords. Claude read-only listing reports 32 installed entries: 30 configured
enabled and two disabled. Cached manifests are not proof of installed/active
connections. No plugin was installed or activated. Reviewed the current request
history and the latest relevant turn of “Βρες plugin για έλεγχο οθόνης”; this is
not an exhaustive review of every stored conversation.

## Sources and implemented transfer

[Shared MIME-info](https://specifications.freedesktop.org/shared-mime-info/latest/ar01s02.html)
describes using an extension hash table for simple suffix patterns and separate
content matching. Our independent transfer is an ordered suffix index in the
existing PBLang file catalog, preserving metadata-only classification and
multipart precedence. It neither imports a MIME database nor claims content
identification. No third-party source was copied.

The complete owning library and existing dashboard/example/tests were reviewed.
The implementation also fixes Windows-path basename disclosure on Linux, using
pure PBLang parsing instead of the host filesystem basename builtin. Existing
CLI summaries, visualization plans and shared web/native/Android UI compilation
consume the same library. UI-record generation is not a real device/OS test.

[Claude marketplaces](https://code.claude.com/docs/en/discover-plugins) separate
catalog discovery and plugin installation.
[Gemini extensions](https://geminicli.com/docs/extensions/) group several kinds
of capabilities and provide inventory commands. Combined with local application
categories, these suggest the following backlog; they are not delivered features:

| Idea | Existing PBLang area | Next concrete evidence needed |
| --- | --- | --- |
| Distinguish discovered/installed/enabled tools | project inventory, PBIDE, pbAI | explicit provenance/state schema and detached UI reports |
| Office document/file navigation | file catalog, sheet libraries | incremental aggregation and cross-renderer interaction tests |
| Media and graphics project inspection | catalog and graphics libraries | metadata parsers, cancellation and per-format tests |
| Game/device/OS-image inventory | emulator and platform libraries | artifact identity and real guest/device execution |

Only names/categories/manifests informed this pass. No automatic conversion of
arbitrary proprietary applications, no polyglot execution, and no access to
EnterpriseCore or Hetzner servers occurred. Native runtime/FFI, composite memory,
audit/release authenticity, PBIDE Run/Designer and real guest-OS coverage remain
open; 1.0.0 remains incomplete.

User constraint reaffirmed during this slice: use independent implementations
of ideas, do not copy discovered code wholesale, and do not execute unknown
code during intake. Tests and defensive review are evidence of this bounded
change, not a guarantee that arbitrary third-party code is safe.
