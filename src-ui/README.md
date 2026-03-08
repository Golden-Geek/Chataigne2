# sv

Everything you need to build a Svelte project, powered by [`sv`](https://github.com/sveltejs/cli).

## Creating a project

If you're seeing this, you've probably already done this step. Congrats!

```sh
# create a new project
npx sv create my-app
```

To recreate this project with the same configuration:

```sh
# recreate this project
npx sv create --template minimal --types ts --install npm chataigne-ui
```

## Developing

Once you've created a project and installed dependencies with `npm install` (or `pnpm install` or `yarn`), start a development server:

```sh
npm run dev

# or start the server and open the app in a new browser tab
npm run dev -- --open
```

Your regular UI dev server remains on `http://127.0.0.1:5173`.

Copilot browser tooling uses a separate dedicated server on `http://127.0.0.1:4173` so it can run in parallel without colliding with your own session.

If you need LAN access instead of loopback-only access:

```sh
npm run dev:lan
```

If you want to start the dedicated Copilot server explicitly:

```sh
npm run dev:copilot
```

## Browser Tooling

The workspace includes a small browser automation toolkit for local UI debugging.

Smoke test the current UI and fail on page errors, request failures, or console errors:

```sh
npm run smoke:ui
```

Inspect rendered DOM for a selector and dump layout-relevant details:

```sh
npm run inspect:ui -- --selector ".dashboard-widget-shell"
```

Useful examples:

```sh
npm run smoke:ui -- --url http://127.0.0.1:4173/?gc_debug_runtime=1
npm run smoke:ui -- --ignore-console-error "ui ws server error"
npm run inspect:ui -- --selector ".dashboard-page" --style display,position,width,height,transform
```

The smoke tool writes a screenshot to `src-ui/artifacts/ui-smoke.png` and a JSON report to `src-ui/artifacts/ui-smoke-report.json`.

## Runtime Probe

Appending `?gc_debug_runtime=1` enables an in-app runtime probe overlay in development. It captures uncaught errors and unhandled promise rejections so runtime failures such as Svelte update-depth errors are surfaced directly in the UI.

This is intended to be used alongside the headless Rust UI backend on `http://localhost:7010` when the page depends on live data.

## Building

To create a production version of your app:

```sh
npm run build
```

You can preview the production build with `npm run preview`.

> To deploy your app, you may need to install an [adapter](https://svelte.dev/docs/kit/adapters) for your target environment.
