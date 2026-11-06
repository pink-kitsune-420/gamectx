# Language adapters (v0.3.0a)

These are thin adapters over the same Rust implementation, not independent ports.
No wrapper changes the Rust core's zero-dependency policy.

| Host | Adapter | Transport |
|---|---|---|
| Python | `python/gamectx_client.py` | Persistent JSON-lines `Client` or ctypes `NativeClient` |
| Node.js / TypeScript | `typescript/gamectx.mjs` and `gamectx.d.mts` | Persistent JSON-lines |
| C / C++ | `native/gamectx.h` | Native library |
| C# | `csharp/GamectxClient.cs` | Native library via P/Invoke |

Use `gamectx serve project.json` for the persistent bridge. Build the native artifact with
`cargo build --manifest-path bindings/native/Cargo.toml --release --locked`, then make
`gamectx_native` discoverable using the host platform's library loader. Keep each handle
alive until every request finishes; release returned strings with the exported free
function. Native callback dispatch stays in the host. The C# adapter takes raw JSON and
returns a raw versioned JSON envelope so the host chooses its own serializer.

See [KIT](../docs/KIT.md) and [FORMAT](../docs/FORMAT.md#integration-kit) for lifecycle,
request fields, operations and limits. Raw C/native JSON preserves number spellings;
Node's parsed JSON numbers follow JavaScript's binary64 rules. IDs should be strings.

Checks: `eval/kit_bindings_check.py` exercises Python and native parity;
`eval/kit_bindings_check.mjs` exercises the Node adapter. `csharp/Check.cs` is a runnable
C# driver. Python can emit its reference fixture replies with `--csharp-fixtures DIR`;
the driver accepts project, requests.txt and expected.txt paths. CI compiles and runs it
on Linux. Actual engine/mobile/Web deployments are not included in these checks.

Optional [routine sessions](../docs/ROUTINES.md) use the same persistent ABI/envelopes.
Python `Character.routine_configure/update/observation/complete/reserve/save/restore` and
Node camelCase equivalents wrap privileged host operations; none is a model tool.
Select only registered `routine_status`/`activity_propose` for the current turn. Supply
current revisions, actor permissions and durable restore watermarks from the engine.
