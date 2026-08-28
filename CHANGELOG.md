## [1.3.5](https://github.com/asepharyana/mytheclipse/compare/v1.3.4...v1.3.5) (2026-08-28)


### Bug Fixes

* **cache:** honor sub-second Redis TTL via PSETEX + document clear() safety ([2c9367a](https://github.com/asepharyana/mytheclipse/commit/2c9367a83c2dd01b1e197ec33215a6c7d3755fa2))

## [1.3.4](https://github.com/asepharyana/mytheclipse/compare/v1.3.3...v1.3.4) (2026-08-28)


### Bug Fixes

* **cache,storage:** harden cache bounds + atomic disk writes ([b6f138b](https://github.com/asepharyana/mytheclipse/commit/b6f138b90d67c9531b5a58993e8ec750e5eec57f))

## [1.3.3](https://github.com/asepharyana/mytheclipse/compare/v1.3.2...v1.3.3) (2026-08-28)


### Bug Fixes

* **publish:** trim mytheclipse keywords to 5 to satisfy crates.io limit ([5f1e3ac](https://github.com/asepharyana/mytheclipse/commit/5f1e3ace5c8cb10881e30f550f51b7d845b24edd))

## [1.3.2](https://github.com/asepharyana/mytheclipse/compare/v1.3.1...v1.3.2) (2026-08-28)


### Bug Fixes

* **ci:** gate cache & storage crate doctests behind their features ([5717f8a](https://github.com/asepharyana/mytheclipse/commit/5717f8aaaae34cb66cdbfc31f4982c93816ee5a3))

## [1.3.1](https://github.com/asepharyana/mytheclipse/compare/v1.3.0...v1.3.1) (2026-08-28)


### Bug Fixes

* **ci:** gate event crate doctest behind mem feature and apply rustfmt ([1994115](https://github.com/asepharyana/mytheclipse/commit/19941156b43ec58370d0d1369174ec84400e9bc9))

# [1.3.0](https://github.com/asepharyana/mytheclipse/compare/v1.2.0...v1.3.0) (2026-08-28)


### Features

* add corex-storage crate for unified storage abstraction ([db4f277](https://github.com/asepharyana/mytheclipse/commit/db4f277336d1e32cb6a2ddd86ac37ae9789fa4f8))

# [1.2.0](https://github.com/asepharyana/mytheclipse/compare/v1.1.0...v1.2.0) (2026-08-28)


### Features

* add panic tracking and logging with PanicTracker ([d119821](https://github.com/asepharyana/mytheclipse/commit/d1198213391710ccc6f2c108b15aa4526380423d))
