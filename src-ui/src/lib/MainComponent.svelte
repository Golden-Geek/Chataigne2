<script lang="ts">
    import { mount, onMount, unmount, type Component } from "svelte";
    import {
        DockviewComponent,
        type CreateComponentOptions,
        type GroupPanelPartInitParameters,
        type IContentRenderer,
        type PanelUpdateEvent,
    } from "dockview-core";
    import "dockview-core/dist/styles/dockview.css";
    import { goldenDockviewTheme } from "$lib/dockview/goldenDockviewTheme";
    import { createGoldenTabRenderer } from "$lib/dockview/createGoldenTabRenderer";

    import type {
        DockPanelExports,
        DockPanelProps,
        DockPanelState,
    } from "$lib/dockview/panel-types";
    import ExplorerPanel from "$lib/panels/ExplorerPanel.svelte";
    import MainViewPanel from "$lib/panels/MainViewPanel.svelte";
    import TerminalPanel from "$lib/panels/TerminalPanel.svelte";
    import UnknownPanel from "$lib/panels/UnknownPanel.svelte";

    type DockPanelComponent = Component<DockPanelProps, DockPanelExports>;
    type MountedDockPanel = DockPanelExports & Record<string, unknown>;

    const panelRegistry: Record<string, DockPanelComponent> = {
        explorer: ExplorerPanel,
        mainView: MainViewPanel,
        terminal: TerminalPanel,
    };

    let containerElement: HTMLDivElement | undefined;
    let dockview: DockviewComponent | undefined;

    const createPanelRenderer = (
        options: CreateComponentOptions,
    ): IContentRenderer => {
        const hostElement = document.createElement("div");
        hostElement.className = "gc-dock-panel-host";

        const PanelComponent = panelRegistry[options.name] ?? UnknownPanel;

        let mountedPanel: MountedDockPanel | undefined;
        let panelState: DockPanelState = {
            panelId: options.id,
            title: options.name,
            params: {},
        };

        return {
            element: hostElement,
            init: (parameters: GroupPanelPartInitParameters): void => {
                panelState = {
                    panelId: parameters.api.id,
                    title: parameters.title ?? options.name,
                    params: parameters.params,
                };

                mountedPanel = mount(PanelComponent, {
                    target: hostElement,
                    props: {
                        api: parameters.api,
                        ...panelState,
                    },
                }) as MountedDockPanel;
            },
            update: (event: PanelUpdateEvent): void => {
                panelState = {
                    ...panelState,
                    params: {
                        ...panelState.params,
                        ...event.params,
                    },
                };

                mountedPanel?.setDockPanelState(panelState);
            },
            dispose: (): void => {
                if (!mountedPanel) {
                    return;
                }

                void unmount(mountedPanel);
                mountedPanel = undefined;
            },
        };
    };

    onMount(() => {
        if (!containerElement) {
            return;
        }

        const rootFontSizePx =
            Number.parseFloat(
                getComputedStyle(document.documentElement).fontSize,
            ) || 16;
        const remToPx = (value: number): number =>
            Math.round(value * rootFontSizePx);

        const containerWidth = containerElement.clientWidth;
        const containerHeight = containerElement.clientHeight;

        const explorerTargetWidth = Math.round(containerWidth * 0.24);
        const explorerMinWidth = remToPx(14);
        const explorerMaxWidth = Math.round(containerWidth * 0.45);
        const clampedExplorerWidth = Math.min(
            Math.max(explorerTargetWidth, explorerMinWidth),
            explorerMaxWidth,
        );
        const mainInitialWidth = Math.max(
            containerWidth - clampedExplorerWidth,
            remToPx(18),
        );

        const terminalInitialHeight = Math.round(containerHeight * 0.28);
        const terminalMinHeight = remToPx(10);
        const terminalMaxHeight = Math.round(containerHeight * 0.55);

        dockview = new DockviewComponent(containerElement, {
            createComponent: createPanelRenderer,
            defaultTabComponent: "golden-tab",
            createTabComponent: (options) =>
                options.name === "golden-tab"
                    ? createGoldenTabRenderer()
                    : undefined,
            theme: goldenDockviewTheme,
        });

        dockview.addPanel({
            id: "panel-explorer",
            title: "Explorer",
            component: "explorer",
            minimumWidth: explorerMinWidth,
            maximumWidth: explorerMaxWidth,
            params: {
                nodes: [
                    "project/",
                    "sequences/",
                    "midi/",
                    "audio/",
                    "presets/",
                ],
            },
        });

        dockview.addPanel({
            id: "panel-main",
            title: "Main View",
            component: "mainView",
            initialWidth: mainInitialWidth,
            params: {
                mode: "graph",
                nodeCount: 3,
            },
            position: { referencePanel: "panel-explorer", direction: "right" },
        });

        dockview.addPanel({
            id: "panel-terminal",
            title: "Terminal",
            component: "terminal",
            initialHeight: terminalInitialHeight,
            minimumHeight: terminalMinHeight,
            maximumHeight: terminalMaxHeight,
            params: {
                lines: [
                    "[info] Session created",
                    "[info] Panels connected",
                    "[warn] No backend signal",
                ],
            },
            position: { referencePanel: "panel-main", direction: "below" },
        });

        return () => {
            dockview?.dispose();
            dockview = undefined;
        };
    });
</script>

<div
    bind:this={containerElement}
    class="gc-dockview"
></div>
