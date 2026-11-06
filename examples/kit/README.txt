This starter uses fictional, scripted observations. They are not model measurements.

With gamectx available on PATH, run:
  gamectx doctor project.json
  gamectx inspect project.json snapshot.json conversation
  gamectx evaluate project.json suite.json
  python3 check.py

Or pass the absolute executable location to check.py as its first argument.

Define data, units, visibility, tools and named recipes once in project.json.
Supply only rows the current actor may access in each snapshot. Snapshot sections can
contain already-filtered memory, facts, events, or other host values.
Schemas/tools are stable and required. Dynamic row/section priorities control dropping.
Host-only fields are removed before selection and SQL. Inspection is host-only too.
Implement tool callbacks in the host and recheck live permissions before any action.

For a persistent bridge, run gamectx serve project.json and write one JSON request
per line. Example:
  {"version":1,"id":"1","op":"doctor","args":{}}
The reply has version, id, ok, and result or error. Build requests use args containing
recipe and snapshot. Query requests additionally supply sql; validate uses name and
arguments. Full protocol, Rust API and language adapters are documented in docs/KIT.md
in the source repository.

Replace suite.json cases with independent expected results from your own game.
Record each candidate's SQL, final answer, actual token counts and latency; null means
unmeasured. Every candidate must include every case exactly once. Comparing recorded
runs performs no API calls. Different recipes can be compared by each run's recipe
override. Guidance is recorded configuration, not applied by this offline scorer.
