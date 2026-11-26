# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

[git-cliff](https://git-cliff.org) writes this file from the commit subjects.
To see the changes that are not in a version, run `git cliff --unreleased`.

## [0.1.3] - 2026-11-26

### Added

- Require api ^0.1.1 in a manifest with an adapter table

### Fixed

- Ask the host for the API version that the extension needs

## [0.1.2] - 2026-11-26

### Added

- Add the TypeScript SDK and its echo example
- Add the Go SDK and its echo example

### Fixed

- Rename the npm package to @nomux-sh/extension

### Documentation

- Specify the answers to a bad first message and a bad frame
- Describe the TypeScript SDK and the conformance suite
- Describe the Go SDK and the requests after a shutdown
## [0.1.1] - 2026-11-26

### Added

- Add the adapter table to the manifest

### Documentation

- State the version rule for additions before 1.0.0
## [0.1.0] - 2026-11-25

### Added

- Add the extension protocol and the nomux-extension crate
- Add the schema and the validator of extension.toml
- Add the echo example extension and its test

### Documentation

- Describe the SDK, the manifest, the layout and the checks
