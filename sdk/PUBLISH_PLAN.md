# Plan: Publish Test Packages to Real Registries

## Context

The SDK packaging (`make pack`, `make test-pack`) is working for Python, JavaScript, and Java. Before pushing the branch, we want to verify the full publish pipeline by publishing test packages with a dummy name (`test-potif-789`) to real package registries. Rust gets dry-run only since path dependencies block crates.io.

## Prerequisites (Manual, One-Time Per Registry)

| Registry | Account Needed | Credential |
|----------|---------------|------------|
| **PyPI** | pypi.org account | API token → `PYPI_TOKEN` env var |
| **npm** | npmjs.com account | `npm login` (interactive) or `NPM_TOKEN` env var |
| **Maven Central** | Sonatype OSSRH account + namespace `io.github.<username>` | GPG key + `OSSRH_USERNAME`/`OSSRH_PASSWORD` env vars |
| **crates.io** | crates.io account | `cargo login` (one-time) |

---

## 1. Python SDK — Publish to PyPI

### File: `sdk/python/pyproject.toml`
- Change `name` → `test-potif-789`
- Add: `authors`, `license`, `readme`, `classifiers`, `[project.urls]`

### File: `sdk/python/Makefile`
- Add `publish` target:
```make
publish: pack
	@python3 -m twine upload dist/*
```

### Nix consideration
- Ensure `twine` is available (add to `flake.nix` or use `pip3 install twine --user`)

### Verify
```bash
make publish                    # uploads to PyPI
pip install test-potif-789      # install from PyPI
python3 -c "from payments import ConnectorClient; print('OK')"
```

---

## 2. JavaScript SDK — Publish to npm

### File: `sdk/javascript/package.json`
- Change `name` → `test-potif-789`
- Add: `author`, `license`, `repository`, `keywords`

### File: `sdk/javascript/Makefile`
- Add `publish` target:
```make
publish: pack
	@cd $(SDK_ROOT) && npm publish --access public
```

### Verify
```bash
npm login                       # one-time
make publish                    # publishes to npm
npm install test-potif-789      # install from npm
node -e "const { ConnectorClient } = require('test-potif-789'); console.log('OK')"
```

---

## 3. Java SDK — Publish to Maven Central (Sonatype OSSRH)

Most complex — requires signed artifacts, full POM metadata, and Sonatype staging workflow.

### File: `sdk/java/build.gradle.kts`
- Change `groupId` → `io.github.<your-username>` (must own namespace on Sonatype)
- Change `artifactId` → `test-potif-789`
- Add `signing` plugin
- Add `io.github.gradle-nexus.publish-plugin` (v2.0.0)
- Add full POM metadata block: `name`, `description`, `url`, `licenses`, `developers`, `scm`
- Add `nexusPublishing` block with Sonatype URLs
- Credentials via env vars: `OSSRH_USERNAME`, `OSSRH_PASSWORD`

### File: `sdk/java/Makefile`
- Add `publish` target:
```make
publish: setup
	@./gradlew publishToSonatype closeAndReleaseSonatypeStagingRepository
```

### Verify
```bash
export OSSRH_USERNAME=... OSSRH_PASSWORD=...
make publish
# In a new Gradle project:
# implementation("io.github.<username>:test-potif-789:0.1.0")
```

---

## 4. Rust SDK — Dry-Run Only

Path dependencies (`ffi = { path = "../../crates/ffi/ffi" }`) prevent real publish.

### File: `sdk/rust/Cargo.toml`
- Change `name` → `test-potif-789`
- Add: `description`, `license`, `authors`, `repository`, `homepage`

### File: `sdk/rust/Makefile` (or add target to existing)
- Add `publish-dry-run` target:
```make
publish-dry-run:
	@cargo publish --dry-run --allow-dirty 2>&1 || \
		echo "Expected: path dependency error. Check metadata validation passed above."
```

### Verify
```bash
cargo publish --dry-run --allow-dirty
# Expected: fails on path deps, but validates metadata completeness
```

---

## Summary of All Changes

| File | Changes |
|------|---------|
| `sdk/python/pyproject.toml` | name → `test-potif-789`, add authors/license/urls/classifiers |
| `sdk/python/Makefile` | add `publish` target (twine upload) |
| `sdk/javascript/package.json` | name → `test-potif-789`, add author/license/repository/keywords |
| `sdk/javascript/Makefile` | add `publish` target (npm publish) |
| `sdk/java/build.gradle.kts` | groupId/artifactId → test names, add signing/nexus/POM metadata |
| `sdk/java/Makefile` | add `publish` target (gradle publishToSonatype) |
| `sdk/rust/Cargo.toml` | name → `test-potif-789`, add description/license/authors/repository |
| `sdk/rust/Makefile` | add `publish-dry-run` target |

## Implementation Order

1. **Python** (simplest — just twine)
2. **JavaScript** (npm publish is straightforward)
3. **Rust** (dry-run only, quick)
4. **Java** (most complex — signing + Sonatype staging)

## Important Caveats

- **Revert names after testing** — change back to `hyperswitch-payments` before real publish
- **Maven Central is permanent** — packages cannot be deleted once released. Consider staging without releasing to test
- **npm unpublish** — allowed within 72 hours for packages with no dependents
- **PyPI** — releases can be deleted from project page, but the version number is permanently consumed

## Verification

For each SDK:
1. `make pack` — ensure artifact builds
2. `make publish` — publish to registry
3. Install from registry in a fresh environment and import the client class
