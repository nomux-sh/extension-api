# AGENTS.md

Rules for coding agents and for people who work in this repository. `CLAUDE.md` and `QWEN.md` refer to this file.

## This repository

`nomux-sh/extension-api` is the nomux extension SDK: the protobuf schemas and the specification of the extension protocol for daemon parts, the Rust crate `nomux-extension` that crates.io publishes, the TypeScript package `@nomux/extension`, the conformance suite of the SDKs, and the JSON Schema of `extension.toml`.

| Path | Responsibility |
|---|---|
| `proto/nomux/extension/v1/` | Protobuf schemas of the package `nomux.extension.v1`: envelope, handshake, life cycle and calls. |
| `spec/extension-protocol.md` | Specification of the extension protocol: framing, limits, life cycle, version rule. |
| `schema/extension.schema.json` | JSON Schema of `extension.toml`. |
| `crates/nomux-extension/` | The published crate: the SDK for the daemon part of an extension. |
| `crates/nomux-extension/src/generated/` | Output of `buf generate`. Committed. Not edited by hand. |
| `crates/nomux-extension/src/manifest.rs`, `src/manifest/` | Types and validator of `extension.toml` (feature `manifest`). |
| `crates/nomux-extension/tests/` | Tests of the protocol, the wire format and the manifest validator. `test_host/` is the host of the tests. |
| `sdk/typescript/` | The TypeScript SDK, package `@nomux/extension`. |
| `sdk/typescript/src/generated/` | Output of `buf generate` for TypeScript. Committed. Not edited by hand. |
| `conformance/` | The conformance suite: the program `nomux-conformance` and the contract of the `echo` extensions (`conformance/README.md`). Not published. |
| `examples/echo/` | Example extension and conformance extension of the Rust SDK, its `extension.toml` and the test that runs the conformance suite against it. |
| `examples/echo-typescript/` | Example extension and conformance extension of the TypeScript SDK, its `extension.toml` and its launcher. |
| `package.json`, `package-lock.json` | The npm workspace of the TypeScript SDK and its example. |
| `tools/protoc-gen-nomux-prost/` | The `buf` plugin that writes the generated Rust code. Not published. |
| `scripts/check-generated.sh` | Fails when the generated code is stale. |
| `buf.yaml`, `buf.gen.yaml` | `buf` module, lint rules, breaking change rules, code generation. |
| `Cargo.toml`, `deny.toml`, `rust-toolchain.toml` | Workspace members and shared versions, license and source policy, pinned toolchain. |
| `cliff.toml`, `CHANGELOG.md` | Changelog configuration and the changelog. |
| `.github/workflows/` | `ci.yml` (checks and the conformance suite), `pull-request.yml` (sign-off, commit messages), `release.yml` (publishes a version tag to crates.io and npm). |

## Rules for this repository

- A field number and an enum value are permanent. Do not change the number or the type of a field. When you delete a field, add a `reserved` statement for its number and its name.
- A change that fails `buf breaking` needs a new package (`nomux.extension.v2`). Do not change the rule set to make the check pass.
- When you change a `.proto` file, run `npm ci` one time, then `buf format -w` and `buf generate`. Commit the schema and the generated code in the same commit.
- Do not edit a file in `crates/nomux-extension/src/generated/` or in `sdk/typescript/src/generated/`.
- The crate `nomux-extension` builds with Rust only. Do not add a `build.rs`, and do not add a step that needs `buf` or `protoc` to a build of the crate.
- `prost`, `prost-build` and `prost-types` have the same version in `Cargo.toml`. Change them together and generate the code again.
- The crate has no async runtime. An extension must be small and start fast. Do not add a dependency to the default features without a measured reason.
- The dependencies of the manifest validator stay behind the feature `manifest`. An extension does not build them.
- `spec/extension-protocol.md`, the schemas and the crate describe one protocol. Change them together.
- `schema/extension.schema.json` and `crates/nomux-extension/src/manifest/` describe one manifest format. Change them together.
- `ApiVersion::CURRENT` is equal to the version of the crate, and `API_VERSION` of `@nomux/extension` is equal to the version of the package. Tests compare them. The two SDKs have the version of the extension API: change them together.
- The TypeScript SDK has one runtime dependency, `@bufbuild/protobuf`. Import only its `wire` module. Do not add a runtime dependency: an extension starts on the first call, and each module adds to the start time.
- A change of the protocol behaviour changes the two SDKs and the conformance suite together. Each SDK passes each case of the suite.
- The example in `README.md`, the example in `crates/nomux-extension/README.md` and `examples/echo/` show one API. Change them together. The TypeScript example in `README.md`, `sdk/typescript/README.md` and `examples/echo-typescript/` show one API. Change them together.
- The UI part of an extension (WASM component, WIT package `nomux:ui`) is not in this repository yet. Do not add a placeholder for it.
- Pin each action in a workflow to a full commit SHA with a version comment.
- Do not publish the crate or the npm package from a computer. The release workflow publishes them from a version tag. The steps are in the [release guide](https://github.com/nomux-sh/.github/blob/main/RELEASING.md).

## Checks

The tools and their versions are in [README.md](README.md#tools). Run these commands from the repository root before a push. CI runs the same commands.

```sh
buf lint
buf format --diff --exit-code
buf build
scripts/check-generated.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo test --workspace --doc
cargo deny check
cargo machete
cargo package -p nomux-extension --locked
actionlint
npm ci
npm run build --workspaces
npm test --workspace @nomux/extension
npm pack --dry-run --workspace @nomux/extension
cargo build -p nomux-conformance -p nomux-echo
target/debug/nomux-conformance --manifest examples/echo/extension.toml -- target/debug/nomux-echo
target/debug/nomux-conformance --manifest examples/echo-typescript/extension.toml -- examples/echo-typescript/bin/nomux-echo
```

If you changed `AGENTS.md`, also run this command with a clone of the [`.github`](https://github.com/nomux-sh/.github) repository next to this repository:

```sh
sh ../.github/standards/sync.sh . && git diff --exit-code
```

<!-- nomux:common:start -->
## Organization rules

These rules apply to each repository of the nomux-sh organization. The source of this block is `standards/AGENTS.common.md` in the [`.github`](https://github.com/nomux-sh/.github) repository. Do not edit the block in other repositories. CI compares it with the source.

### Before you change something

- Read the code around the change. Use the names, the structure and the style that the repository already has.
- Use the terms of the [glossary](https://github.com/nomux-sh/docs/blob/main/src/content/docs/reference/glossary.md). One concept has one name in code, CLI, UI and documentation.
- An architecture change needs an ADR in the [`docs`](https://github.com/nomux-sh/docs) repository before the implementation.
- Check versions, APIs and file paths before you use them. Do not guess.

### Scope

- Make only the change that the task asks for. Do not add options, abstractions or helpers that the task does not need.
- Do not add a placeholder that looks like an implementation. If work is deferred, say it in the pull request.
- Validate input at system boundaries. Trust internal code.

### Code

- Keep it simple (KISS). Build only what the task needs now (YAGNI). Have one source for each piece of logic (DRY).
- Write modern, readable code. Do not write clever code that the reader must decode.
- One type, one module and one file have one responsibility. Do not make a type that knows or does everything. Split it before it grows.
- Keep the file tree tidy. A new file goes where its neighbours have the same responsibility. Do not make catch-all directories or files such as `utils`, `common`, `helpers` or `misc`.
- Give each file, type and function a name that says what it is or does. Do not use abbreviations that the glossary does not have.
- Keep functions short and at one level of abstraction. Do not nest deeply. Return early.
- Do not duplicate code. If the same logic is necessary in a second place, move it to one place and call it.
- Comments: none by default. Write one line only for a reason that the code cannot show.
- Do not swallow errors. Do not add retries without a known cause.
- The performance targets of the project come before features. A change that breaks a target is not merged.
- Do not copy code from GPL, AGPL or Commons Clause projects.

### Tests

- Complete the code first. Write the tests last, and run them when the work is complete.
- Write the tests that matter: edge cases, behaviour that can break later, and the proof that the current behaviour is correct.
- Do not write a test only to have a test. Do not test what is obvious or what the compiler already guarantees.
- A test must fail when the behaviour that it guards is removed or moved.
- Do not mock the thing that the test examines.

### CI

- CI must be fast. Reviews and CI are the only things that slow a merge.
- Cache dependencies and build output. Run independent jobs in parallel. Cancel a run when a newer commit arrives.
- Put slow work (long fuzz runs, benchmarks, full matrix builds) in scheduled runs, not in pull request runs.
- When you add a CI step, measure the time that it adds and say it in the pull request.

### Text

- Write code, comments, documentation, commit messages and pull requests in English.
- Use short sentences, the active voice and one word for one concept. Put the condition before the instruction.
- Do not use these words: delve, robust, seamless, seamlessly, leverage, utilize, harness, pivotal, intricate, furthermore, moreover, notably, cutting-edge, best-in-class.

### Commits

- One subject line: `type: description`. No scope. 72 characters at most.
- Types: `build`, `chore`, `ci`, `docs`, `feat`, `fix`, `perf`, `refactor`, `revert`, `style`, `test`. Add `!` before the colon for a breaking change.
- The description says what changed, in plain English. Do not name a phase, a milestone or a ticket.
- Sign off each commit with `git commit -s`. `Signed-off-by` is the only trailer.
- Do not add `Co-authored-by`, a tool name or a "generated with" line to a commit or to a pull request.
- Enable the commit hook one time in each clone: `git config core.hooksPath .githooks`.

### Pull requests

- One logical change in each pull request. The branch name is short and names the change.
- The title has the format of a commit subject, 70 characters at most.
- The description says what changed and why, then shows the commands that you ran and their real result.
- Each commit in the pull request obeys the commit rules. The maintainers merge with a rebase, thus each commit goes to `main` unchanged.
- Do not merge when a check fails. Do not disable a check to make it pass.

### Never commit

- Secrets, tokens, keys or passwords.
- Paths, host names or account names of a local machine.
- Generated build output, editor files or local configuration.

### Before you push

1. Run the checks that the "Checks" section of this file lists.
2. Read your diff from top to bottom.
3. Make sure that the changelog and the documentation agree with the change.
<!-- nomux:common:end -->
