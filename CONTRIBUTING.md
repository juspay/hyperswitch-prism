# Contributing to Hyperswitch Prism

First off, thank you for considering contributing to Hyperswitch Prism! It's people like you that make Hyperswitch such a great tool.

## Code of Conduct

By participating in this project, you are expected to uphold our [Code of Conduct](./docs/CODE_OF_CONDUCT.md).

## Getting Started

Hyperswitch Prism is a multi-language SDK project powered by a core Rust implementation and UniFFI bindings.

### Prerequisites

Depending on which part of the project you want to contribute to, you will need:

- **Core**: Rust (latest stable)
- **Node.js SDK**: Node.js 18+, npm
- **Python SDK**: Python 3.8+, pip, uv (recommended)
- **Java SDK**: Java 11+, Gradle

### Local Setup

1. Fork and clone the repository.
2. Initialize submodules (if any).
3. Navigate to the language SDK you want to work on (e.g., `cd sdk/javascript`).
4. Follow the language-specific setup instructions in their respective `README.md` files.

For example, to set up the Node.js SDK:
```bash
cd sdk/javascript
npm install
npm run build
```

## How to Contribute

### 1. Find an Issue
Look for open issues labeled `good first issue` or `help wanted`. If you want to work on something else, please open an issue first to discuss it with the maintainers.

### 2. Create a Branch
Create a branch for your changes:
```bash
git checkout -b feature/your-feature-name
```
Or for bugs:
```bash
git checkout -b fix/your-bug-fix
```

### 3. Make Changes
- Write clear, concise, and documented code.
- Ensure your code follows the existing style of the codebase.
- Add tests for any new features or bug fixes.

### 4. Run Tests
Ensure all existing tests pass and your new tests run successfully.
For the JS SDK, you can run the smoke tests:
```bash
npm run test
```

### 5. Commit Your Changes
We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification for commit messages.
- `feat:` for new features
- `fix:` for bug fixes
- `docs:` for documentation changes
- `test:` for adding or modifying tests
- `refactor:` for code refactoring

Example:
```bash
git commit -m "feat(sdk/javascript): add new connector support"
```

### 6. Open a Pull Request
- Push your branch to your fork.
- Open a Pull Request against the `main` branch of the `juspay/hyperswitch-prism` repository.
- Fill out the PR template completely.
- Reference any related issues (e.g., `Fixes #123`).

## Coding Standards

- **Rust**: Use `rustfmt` and `clippy`.
- **TypeScript**: We use strict mode. Ensure all types are properly defined. Avoid `any` where possible.
- **Python**: Use type hints, `black` for formatting, and `flake8` for linting.

## License

By contributing, you agree that your contributions will be licensed under its Apache 2.0 License.
