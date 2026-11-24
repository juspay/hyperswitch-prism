# Lean spec

Lean 4 proofs about payment behaviour. Right now it holds one module,
`PrismSpec/Authorize.lean`, which defines `PaymentStatus`, a simple `authorize`
function, and `authorize_returns_valid_status`. That theorem proves `authorize`
only ever returns `authorized` or `pending`.

## Layout

| Path | Purpose |
|---|---|
| `lakefile.toml` | Lake project config (one library: `PrismSpec`) |
| `lean-toolchain` | Pinned Lean version (`leanprover/lean4:v4.22.0`) |
| `PrismSpec.lean` | Library root; imports every module |
| `PrismSpec/Authorize.lean` | `PaymentStatus`, `authorize`, and its theorem |

## Setup

Install `elan`, the Lean toolchain manager. It reads `lean-toolchain` and
fetches the right Lean version automatically.

```sh
# macOS
brew install elan-init
# or any platform
curl https://elan.lean-lang.org/elan-init.sh -sSf | sh
```

## Commands

Run these from `tests/lean/`:

```sh
lake build                               # type-check every module and proof
lake env lean PrismSpec/Authorize.lean   # check a single file
lake clean                               # delete build output (.lake/)
```

A successful `lake build` ends with `Build completed successfully.`. If a proof
fails, the build fails.

## Adding a module

1. Create `PrismSpec/<Name>.lean`.
2. Add `import PrismSpec.<Name>` to `PrismSpec.lean`.
3. Run `lake build`.
