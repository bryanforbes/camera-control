# AGENTS.md

## Project Overview

Camera Control is a Tauri-based desktop application for controlling AViPAS cameras at CrossLife Community Church. The project combines:

- **Frontend**: SvelteKit 2 with TypeScript, Svelte 5 (with runes), and Tailwind CSS 4
- **Backend**: Rust with Tauri 2, providing native camera control via serial port communication
- **Protocols**: Pelco-D and VISCA camera control protocols
- **Build System**: Vite for frontend, Cargo for Rust backend
- **Package Manager**: pnpm, with the version pinned in `mise.toml` and `package.json`

The application is cross-platform, targeting macOS (Universal), Windows, and Linux.

## Setup Commands

### Prerequisites

- mise for managing Node.js, pnpm, Rust, and development tools
- Platform-specific dependencies:
  - **macOS**: Xcode Command Line Tools
  - **Linux**: `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf xdg-utils libudev-dev`
  - **Windows**: WebView2 (usually pre-installed on Windows 10/11)

### Initial Setup

```bash
# Install the project's tools
mise install --locked

# On macOS, use this command to include both Rust targets for universal builds
mise -E macos install --locked

# Alternatively, add MISE_ENV=macos to your shell environment for macOS-specific setup
```

The mise postinstall hook installs frontend dependencies and runs `hk install --mise`. To reinstall Git hooks separately, run `mise exec -- hk install --mise`.

## Development Workflow

### Starting Development Server

```bash
# Run the Tauri app in development mode (starts both frontend and backend)
mise dev

# Alternative: Run frontend dev server only
pnpm dev
```

The development server runs on port 1420 (frontend) and 1421 (HMR). Tauri expects these fixed ports.

### Type Checking

```bash
# Run Svelte type checking
pnpm check

# Run Svelte type checking in watch mode
pnpm check:watch
```

### Preview Production Build

```bash
pnpm preview
```

## Building

### Frontend Build

```bash
# Build the frontend (static site generation)
pnpm build
```

The build output goes to `.svelte-kit/` and `build/` directories.

### Tauri Application Build

```bash
# Build for current platform
mise build
```

Build artifacts are placed in `src-tauri/target/release/`.

## Testing Instructions

### Rust Unit Tests

```bash
# Run all Rust unit tests
mise test

# Run with output visible
mise test -- -- --nocapture
```

**Note**: There are no frontend tests currently. Focus on Rust backend tests.

## Code Style and Linting

### Linting Commands

```bash
# Run the full hk check suite
mise lint --all

# Run frontend linters only
mise lint-frontend --all

# Run backend linters only
mise lint-backend --all

# Individual linters
mise lint --step prettier --all
mise lint --step eslint --all
mise lint --step stylelint --all

# Rust linting
mise lint --step clippy --all

# Rust formatting check
mise lint --step rustfmt --all
```

### Formatting Commands

```bash
# Format all files
mist fix --step prettier --all

# Format Rust files
mise fix --step rustfmt --all
```

### Pre-commit Hooks

Git hooks are defined in `hk.pkl`. The pre-commit hook runs the configured checks on matching staged files without applying fixes. These include frontend type checking and linting, Rustfmt and Clippy, and checks for GitHub Actions and Pkl files. The commit-msg hook runs Commitlint.

### Code Style Guidelines

**TypeScript/Svelte**:

- Single quotes for strings
- Experimental ternaries enabled
- Organize imports automatically
- Tailwind class sorting enabled
- Strict TypeScript checking
- Prefer consistent type imports/exports (`import type`)
- Naming convention enforcement in `src/` directory
- Svelte 5 runes mode enabled

**Rust**:

- Standard Rust formatting (rustfmt)
- Clippy linting enforced
- Follow `rustfmt.toml` configuration

**File Organization**:

- Frontend code: `src/` directory
- Backend code: `src-tauri/src/` directory
- Routes: `src/routes/`
- Shared libraries: `src/lib/`
- Generated bindings: `src/lib/bindings.ts` (auto-generated, don't edit)

## CI/CD Pipeline

### GitHub Actions Workflows

**Test Workflow** (`.github/workflows/test.yaml`):

- Triggers on push to `master` and PRs
- Runs on macOS, Ubuntu, and Windows
- Runs the full hk check suite with `mise lint --all` on Ubuntu
- Runs Rust unit tests with `mise test` and builds the Tauri app on all three platforms
- Caches verified mise downloads, the pnpm store, and Cargo dependencies and build artifacts

**Release Workflow** (`.github/workflows/release.yaml`):

- Handles versioning and releases
- Creates signed builds for distribution on `v*` tag pushes with GitHub Actions caching disabled

Both workflows use `jdx/mise-action` to install tools from the committed mise configuration. They load the `ci` environment to skip local hooks and use the Tauri CLI from the pnpm dependencies. macOS jobs also load the `macos` environment for both Rust targets.

### Running CI Checks Locally

```bash
mise lint --all
mise test
mise build

# On macOS, build the universal app
mise -E macos build
```

## Commit Guidelines

### Commit Message Format

The project uses Conventional Commits via commitlint:

```text
type(scope): subject

body

footer
```

**Types**: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, etc.

**Example**:

```text
feat(camera): add zoom control support

Implement zoom in/out commands for VISCA protocol

Closes #123
```

Commits are validated using `@commitlint/config-conventional`.

## Pull Request Guidelines

Before submitting a PR:

1. Run linting and type checking with `mise lint --all`.
2. Run tests with `mise test`.
3. Verify the desktop build with `mise build` or at minimum the frontend build with `pnpm build`.

All checks must pass before merging.

## Project Structure

```text
camera-control/
├── src/                      # Frontend source (SvelteKit + Svelte 5)
│   ├── lib/                  # Shared libraries and components
│   │   └── bindings.ts       # Auto-generated Tauri bindings (DO NOT EDIT)
│   ├── routes/               # SvelteKit routes
│   ├── app.html              # HTML template
│   └── *.css                 # Styles
├── src-tauri/                # Rust backend (Tauri)
│   ├── src/
│   │   ├── main.rs           # Entry point
│   │   ├── camera.rs         # Camera trait and abstractions
│   │   ├── pelco_camera.rs   # Pelco-D protocol implementation
│   │   ├── visca/            # VISCA protocol implementation
│   │   ├── ui_state.rs       # UI state management
│   │   └── error.rs          # Error types
│   ├── Cargo.toml            # Rust dependencies
│   ├── Tauri.toml            # Tauri configuration
│   └── build.rs              # Build script
├── .github/workflows/        # CI/CD pipelines
├── package.json              # Frontend dependencies and scripts
└── pnpm-lock.yaml            # Lockfile (always commit)
```

## Technology-Specific Notes

### SvelteKit + Tauri Integration

- Uses `@sveltejs/adapter-static` for SSG (no SSR support in Tauri)
- All routes are pre-rendered at build time
- Frontend communicates with Rust backend via Tauri commands
- Type-safe bindings generated using `tauri-specta`

### Rust Backend

- Uses `serialport` crate for serial communication
- Implements two camera protocols: Pelco-D and VISCA
- State management via `tauri-plugin-store`
- Auto-updates via `tauri-plugin-updater`
- Single instance enforcement via `tauri-plugin-single-instance`

### Vite Configuration

- Fixed ports: 1420 (dev server), 1421 (HMR)
- Watches exclude `src-tauri/` to avoid conflicts
- Integrates Tailwind CSS 4 via Vite plugin

## Debugging and Troubleshooting

### Common Issues

**Port Already in Use**:

```bash
# Kill process on port 1420
lsof -ti:1420 | xargs kill -9
```

**Rust Compilation Errors**:

```bash
# Clean Rust build cache
cargo clean --manifest-path src-tauri/Cargo.toml

# Update dependencies
cargo update --manifest-path src-tauri/Cargo.toml
```

**Frontend Type Errors**:

```bash
# Regenerate SvelteKit files
pnpm exec svelte-kit sync

# Clean and reinstall
rm -rf node_modules .svelte-kit && mise install
```

### Development Tips

- Use `cargo watch` for faster Rust iteration
- Generated TypeScript bindings at `src/lib/bindings.ts` are created by `tauri-specta` - regenerate with Rust builds
- Vite dev server supports HMR for Svelte components
- Check Tauri logs in the development console for backend debugging

## Security Considerations

- Application uses code signing for releases (via `TAURI_SIGNING_PRIVATE_KEY`)
- Secrets are stored in GitHub Secrets, never committed
- Serial port access requires appropriate permissions on each platform
- Updates are signed and verified via `tauri-plugin-updater`

## Environment Variables

**Development**:

- `TAURI_DEV_HOST`: Custom host for development server (optional)
- `RUST_LOG`: Control Rust logging level (e.g., `RUST_LOG=debug`)

**Build/Release**:

- `GITHUB_TOKEN`: Required for release workflow
- `TAURI_SIGNING_PRIVATE_KEY`: Code signing key
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: Code signing password

## Additional Resources

- [Tauri Documentation](https://v2.tauri.app/)
- [SvelteKit Documentation](https://kit.svelte.dev/)
- [Svelte 5 Documentation](https://svelte.dev/)
- [Pelco-D Protocol](https://github.com/bryanforbes/pelcodrs)
