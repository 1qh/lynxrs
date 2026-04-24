# cargo-mutants sanity run — src/auth.rs (shard 0/20)

Command: `cargo mutants --file src/auth.rs --in-place --shard 0/20 --timeout=120`

Result: 5 mutants tested, **5 unviable** (compile-error mutations — type system
caught them before tests ran). 0 caught, 0 missed, 0 timeouts.

Unviable mutations mostly replace `UserDto::from -> Self` and handler return bodies
with `Default::default()` — `UserDto` does not derive `Default`, so rustc rejects
the mutation at compile time. That's a silent strength of the current code: strong
newtype typing means many boneheaded edits don't even build.

Full shard sweep (all 20 shards) is tracked as follow-up — it needs ~30 min of CI
and access to a running postgres container per mutant (integration tests).
