# Changelog

While the version is `0.x` or a pre-release, any release may change the API,
command-line options or packed formats. Every entry carries one label so projects
know what an upgrade costs:

| Label | Meaning | What projects using gamectx should do |
|---|---|---|
| **[format]** | Packed output or accepted input changes; one or more `conformance/` expected files changed. | Update ports, rerun conformance, and recheck model decisions that depend on the changed shape. |
| **[api]** | Rust library or command-line interface changes; output is the same. | Update calling code. |
| **[internal]** | Faster, safer or clearer with identical behavior; conformance files untouched. | Upgrade freely. |
| **[docs]** | Documentation only. | Nothing. |

## 0.1.0-alpha.1 — 2026-11-05

First public release.

- **[format]** `json`: strict, order-preserving parsing that keeps exact number text; compact serialization rules.
- **[format]** `table`: lossless header-plus-rows packing, optionally grouped by one column (`group_by`).
- **[format]** `view`: `sort_by`, `limit`, `fields` and `round` shape table rows before packing;
  `verify` reports viewed sections separately from exact round-trips.
- **[format]** `schema`: lossless tool-schema packing with shared `$defs`; lossy `outline` signatures.
- **[format]** `catalog`: searchable, paged registries with change-detecting generations.
- **[format]** `budget`: stable-first assembly that drops low-priority sections, with receipts.
- **[format]** `estimate`: conservative token estimates calibrated from provider usage reports.
- **[api]** `gamectx` command: `pack`, `report`, `verify`, `search`, `outline`, `calibrate`.
- **[docs]** 51 shared conformance cases defining every format, for ports in other languages.
- **[docs]** Model evaluation script (`eval/`), measured results in the README, and
  `docs/GUIDE.md` with practical rules drawn from them.
