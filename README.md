# bedrockRinth

`bedrockRinth` is a Bedrock-oriented launcher and ecosystem experiment based on a normalized import of the Modrinth monorepo.

This repository is no longer a raw folder dump. The real project root now lives at the repository root, with:

- `apps/app` for the desktop launcher shell
- `apps/app-frontend` for the frontend
- `packages/*` for shared libraries
- `.github/workflows` for CI

## What this fork is for

- exploring Bedrock launcher flows on top of the Modrinth desktop stack
- testing Bedrock-specific UX and onboarding
- keeping a buildable monorepo layout instead of a nested source import

## Status

Experimental fork. The structure is valid and CI smoke builds are enabled, but this is not yet a polished standalone product.

## Stack

- Rust
- Tauri
- Vue
- pnpm workspace
- Turbo monorepo

## Local development

Requirements:

- Node.js `20.19.2`
- `pnpm`
- Rust toolchain
- Tauri prerequisites

Install and run:

```bash
pnpm install
cp packages/app-lib/.env.staging packages/app-lib/.env
pnpm app:dev
```

## CI

This fork includes a dedicated smoke-build workflow that:

- installs workspace dependencies
- prepares the app environment
- builds the frontend package
- runs `cargo check -p theseus_gui`

That gives a stable signal that the repository is structured like a real project and not just a nested archive.
