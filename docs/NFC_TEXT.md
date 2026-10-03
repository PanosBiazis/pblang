# NFC NDEF Text data

`stdlib:nfc_text.sapp` implements a bounded single-record NDEF RTD Text subset
in PBLang. `encode(text, language="en")` returns bytes; `inspect(bytes,
include_text=false)` validates and returns metadata. Text is returned only on
explicit opt-in. Nothing is stored, executed, opened or written to hardware.

Supported: MB+ME, TNF_WELL_KNOWN, type T, UTF-8, short or four-byte payload length.
Input message is at most4096bytes; text encoder limit4000UTF-8bytes; language
is1..15ASCII letters/hyphens without empty components. This is a restricted
language label, not a full BCP47 validator. Invalid UTF-8, trailing bytes,
overflowed/truncated lengths, ID fields, chunking, multiple records, URI and
UTF-16 are refused. NFC URI/MIME/SmartPoster/TLV/physical tag memory layouts
are not covered. No signature, tag authenticity or ownership is inferred.

The known vector for text `Hi`, language `en` is
`D1 01 05 54 02 65 6E 48 69`. Tests independently assert this vector, Unicode,
long records, empty text, privacy defaults and malformed input.
Run `examples/nfc_text.sapp` for synthetic encoding/inspection.

An OS NFC adapter, reader access grant, explicit write confirmation and
independent hardware qualification are still needed for actual tag I/O.
No Android/iOS reader session, PC/SC, PN532/Arduino SPI/I2C, NDEF write, tag lock,
credential emulation or Xiaomi Tag cloud-location connector is supplied here.

References: [Android NDEF Text guidance](https://developer.android.com/develop/connectivity/nfc/nfc),
[NdefRecord contract](https://developer.android.com/reference/android/nfc/NdefRecord).
Implementation is original; no external source code was copied.
