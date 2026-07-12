<script lang="ts">
	import { mount, onMount, unmount } from 'svelte';
	import {
		DockviewComponent,
		type AddPanelOptions,
		type CreateComponentOptions,
		type DockviewGroupPanel,
		type DockviewIDisposable,
		type GroupPanelPartInitParameters,
		type IContentRenderer,
		type IGroupHeaderProps,
		type IHeaderActionsRenderer,
		type IDockviewPanel,
		type PanelUpdateEvent,
		type SerializedDockview
	} from 'dockview-core';

	import 'dockview-core/dist/styles/dockview.css';
	import { goldenDockviewTheme } from '../../dockview/goldenDockviewTheme';
	import { createGoldenTabRenderer } from '../../dockview/createGoldenTabRenderer';

	import type {
		PanelApi,
		PanelController,
		PanelDefinition,
		PanelExports,
		PanelHandle,
		PanelParams,
		PanelQuery,
		PanelSpawnPosition,
		PanelSpawnRequest,
		PanelState,
		UserPanelDefinition,
		UserPanelDefinitionMap
	} from '../../dockview/panel-types';
	import { configureNodeIcons, type NodeIconSet } from '../../store/node-types';
	import { registerProjectUiStateHost } from '../../store/project-ui-state';
	import { loadPersistedDockLayout, savePersistedDockLayout } from '../../store/ui-persistence';
	import { appState } from '../../store/workbench.svelte';

	import UnknownPanel from '../panels/UnknownPanel.svelte';
	import Outliner from '../panels/outliner/OutlinerPanel.svelte';
	import InspectorPanel from '../panels/inspector/InspectorPanel.svelte';
	import LoggerPanel from '../panels/logger/LoggerPanel.svelte';
	import WarningsPanel from '../panels/warnings/WarningsPanel.svelte';
	import DashboardPanel from '../panels/dashboard/DashboardPanel.svelte';
	import StreamDeckEditorPanel from '../panels/module-editors/StreamDeckEditorPanel.svelte';
	import GroupPanelAddAction from './GroupPanelAddAction.svelte';

	let { userPanels, initialPanels, nodeIcons } = $props<{
		userPanels?: UserPanelDefinitionMap;
		initialPanels?: PanelSpawnRequest[];
		nodeIcons?: NodeIconSet;
	}>();

	$effect(() => {
		configureNodeIcons(nodeIcons);
	});

	type MountedPanel = PanelExports & Record<string, unknown>;
	type MountedHeaderAction = {
		closeMenu?: () => void;
	};

	const goldenPanelDefinitions: Record<string, PanelDefinition> = {
		inspector: {
			panelType: 'inspector',
			title: 'Inspector',
			component: InspectorPanel,
			origin: 'golden'
		},

		outliner: {
			panelType: 'outliner',
			title: 'Outliner',
			component: Outliner,
			origin: 'golden'
		},
		dashboard: {
			panelType: 'dashboard',
			title: 'Dashboard',
			component: DashboardPanel,
			origin: 'golden'
		},
		logger: {
			panelType: 'logger',
			title: 'Logger',
			component: LoggerPanel,
			origin: 'golden'
		},
		warnings: {
			panelType: 'warnings',
			title: 'Warnings',
			component: WarningsPanel,
			origin: 'golden'
		},
		'streamdeck-editor': {
			panelType: 'streamdeck-editor',
			title: 'Stream Deck',
			description: 'Visual editor for Stream Deck modules.',
			component: StreamDeckEditorPanel,
			category: 'Module Editors',
			origin: 'golden'
		}
	};

	const formatPanelTypeTitle = (panelType: string): string =>
		panelType
			.replace(/([a-z0-9])([A-Z])/g, '$1 $2')
			.replace(/[_-]+/g, ' ')
			.replace(/\s+/g, ' ')
			.trim()
			.replace(/^./, (match) => match.toUpperCase());

	const isUserPanelDefinition = (
		value: UserPanelDefinitionMap[string]
	): value is UserPanelDefinition =>
		typeof value === 'object' && value !== null && 'component' in value;

	const normalizeUserPanels = (
		entries: UserPanelDefinitionMap | undefined
	): Record<string, PanelDefinition> => {
		if (!entries) {
			return {};
		}

		return Object.entries(entries).reduce<Record<string, PanelDefinition>>(
			(accumulator, [panelType, entry]) => {
				if (isUserPanelDefinition(entry)) {
					accumulator[panelType] = {
						panelType,
						title: entry.title ?? formatPanelTypeTitle(panelType),
						component: entry.component,
						description: entry.description,
						defaultParams: entry.defaultParams,
						renderPolicy: entry.renderPolicy,
						category: entry.category,
						origin: 'user'
					};
					return accumulator;
				}

				accumulator[panelType] = {
					panelType,
					title: formatPanelTypeTitle(panelType),
					component: entry,
					origin: 'user'
				};
				return accumulator;
			},
			{}
		);
	};

	const panelDefinitions = $derived<Record<string, PanelDefinition>>({
		...goldenPanelDefinitions,
		...normalizeUserPanels(userPanels)
	});
	const availablePanelDefinitions = $derived(
		Object.values(panelDefinitions).sort((first, second) => first.title.localeCompare(second.title))
	);

	let containerElement: HTMLDivElement | undefined;
	let dockview: DockviewComponent | undefined;
	let isRestoringPersistedLayout = false;

	const panelInstanceCounters = new Map<string, number>();

	const nextPanelId = (panelType: string): string => {
		let nextIndex = panelInstanceCounters.get(panelType) ?? 1;
		let panelId = `${panelType}-${nextIndex}`;

		while (dockview?.getGroupPanel(panelId)) {
			nextIndex += 1;
			panelId = `${panelType}-${nextIndex}`;
		}

		panelInstanceCounters.set(panelType, nextIndex + 1);
		return panelId;
	};

	const toDockviewPosition = (
		position: PanelSpawnPosition | undefined
	): AddPanelOptions['position'] | undefined => {
		if (!position) {
			return undefined;
		}

		const direction = position.direction ?? 'within';
		if (position.referencePanelId) {
			return {
				referencePanel: position.referencePanelId,
				direction
			};
		}

		if (direction === 'within') {
			return undefined;
		}

		return { direction };
	};

	const createPanelRenderer = (options: CreateComponentOptions): IContentRenderer => {
		const hostElement = document.createElement('div');
		hostElement.className = 'gc-dock-panel-host';

		const panelDefinition = panelDefinitions[options.name];
		const PanelComponent = panelDefinition?.component ?? UnknownPanel;

		let mountedPanel: MountedPanel | undefined;
		let panelState: PanelState = {
			panelId: options.id,
			panelType: options.name,
			title: panelDefinition?.title ?? formatPanelTypeTitle(options.name),
			params: {}
		};

		return {
			element: hostElement,
			init: (parameters: GroupPanelPartInitParameters): void => {
				const generatedTitle = panelDefinition?.title ?? formatPanelTypeTitle(options.name);
				const rawTitle = typeof parameters.title === 'string' ? parameters.title.trim() : '';
				const isLikelyDockviewFallback =
					rawTitle.length > 0 &&
					rawTitle.toLowerCase() === options.name.toLowerCase() &&
					rawTitle === rawTitle.toLowerCase();
				const resolvedTitle =
					rawTitle.length === 0 || isLikelyDockviewFallback ? generatedTitle : rawTitle;
				if (parameters.title !== resolvedTitle) {
					parameters.api.setTitle(resolvedTitle);
				}

				panelState = {
					panelId: parameters.api.id,
					panelType: options.name,
					title: resolvedTitle,
					params: parameters.params ?? {}
				};

				const panelApi: PanelApi = {
					setTitle: (nextTitle: string): void => {
						parameters.api.setTitle(nextTitle);
					},
					close: (): void => {
						parameters.api.close();
					},
					updateParams: (params: PanelParams): void => {
						parameters.api.updateParameters(params);
					},
					getParams: <T extends PanelParams = PanelParams>(): T => {
						return parameters.api.getParameters<T>();
					}
				};

				mountedPanel = mount(PanelComponent, {
					target: hostElement,
					props: {
						panelApi,
						...panelState
					}
				}) as MountedPanel;
			},
			update: (event: PanelUpdateEvent): void => {
				panelState = {
					...panelState,
					params: {
						...panelState.params,
						...(event.params ?? {})
					}
				};

				mountedPanel?.setPanelState(panelState);
			},
			dispose: (): void => {
				if (!mountedPanel) {
					return;
				}

				void unmount(mountedPanel);
				mountedPanel = undefined;
			}
		};
	};

	const addPanel = (
		request: PanelSpawnRequest,
		overridePosition?: AddPanelOptions['position']
	): string | null => {
		if (!dockview) {
			return null;
		}

		const panelDefinition = panelDefinitions[request.panelType];
		if (!panelDefinition) {
			console.error(`Panel "${request.panelType}" is not registered.`);
			return null;
		}

		const panelId = request.panelId ?? nextPanelId(request.panelType);
		const options: AddPanelOptions = {
			id: panelId,
			component: request.panelType,
			title: request.title ?? panelDefinition.title,
			params: {
				...(panelDefinition.defaultParams ?? {}),
				...(request.params ?? {})
			},
			renderer: request.renderPolicy ?? panelDefinition.renderPolicy,
			inactive: request.inactive,
			initialWidth: request.initialWidth,
			initialHeight: request.initialHeight,
			minimumWidth: request.minimumWidth,
			maximumWidth: request.maximumWidth,
			minimumHeight: request.minimumHeight,
			maximumHeight: request.maximumHeight
		};

		const position = overridePosition ?? toDockviewPosition(request.position);
		if (position) {
			options.position = position;
		}

		dockview.addPanel(options);
		return panelId;
	};

	const findPanelByType = (panelType: string): IDockviewPanel | undefined => {
		return dockview?.panels.find((panel) => panel.api.component === panelType);
	};

	const resolvePanelFromQuery = (query: PanelQuery): IDockviewPanel | undefined => {
		if (!dockview) {
			return undefined;
		}

		if (query.panelId) {
			return dockview.getGroupPanel(query.panelId);
		}

		if (query.panelType) {
			return findPanelByType(query.panelType);
		}

		return undefined;
	};

	const asPanelHandle = (panel: IDockviewPanel): PanelHandle => ({
		panelId: panel.id,
		panelType: panel.api.component,
		getTitle: (): string => panel.title ?? panel.api.component,
		isActive: (): boolean => panel.api.isActive,
		setActive: (): void => panel.api.setActive(),
		close: (): void => panel.api.close(),
		setTitle: (title: string): void => panel.setTitle(title),
		updateParams: (params: PanelParams): void => panel.api.updateParameters(params),
		getParams: <T extends PanelParams = PanelParams>(): T => panel.api.getParameters<T>()
	});

	const defaultPanelRequestFor = (request: PanelSpawnRequest): PanelSpawnRequest | undefined => {
		const initialPanelRequests: PanelSpawnRequest[] = initialPanels ?? createDefaultInitialPanels();
		if (request.panelId) {
			const panelById = initialPanelRequests.find((panel) => panel.panelId === request.panelId);
			if (panelById) {
				return panelById;
			}
		}
		return initialPanelRequests.find((panel) => panel.panelType === request.panelType);
	};

	const withPanelDefaults = (request: PanelSpawnRequest): PanelSpawnRequest => {
		const defaults = defaultPanelRequestFor(request);
		if (!defaults) {
			return {
				...request,
				inactive: request.inactive ?? false
			};
		}

		return {
			panelType: request.panelType,
			required: request.required ?? defaults.required,
			panelId: request.panelId ?? defaults.panelId,
			title: request.title ?? defaults.title,
			params: {
				...(defaults.params ?? {}),
				...(request.params ?? {})
			},
			renderPolicy: request.renderPolicy ?? defaults.renderPolicy,
			position: request.position ?? defaults.position,
			initialWidth: request.initialWidth ?? defaults.initialWidth,
			initialHeight: request.initialHeight ?? defaults.initialHeight,
			minimumWidth: request.minimumWidth ?? defaults.minimumWidth,
			maximumWidth: request.maximumWidth ?? defaults.maximumWidth,
			minimumHeight: request.minimumHeight ?? defaults.minimumHeight,
			maximumHeight: request.maximumHeight ?? defaults.maximumHeight,
			inactive: request.inactive ?? false
		};
	};

	const accessPanel = (query: PanelQuery): PanelHandle | null => {
		const panel = resolvePanelFromQuery(query);
		return panel ? asPanelHandle(panel) : null;
	};

	const getPanelById = (panelId: string): PanelHandle | null => {
		const panel = resolvePanelFromQuery({ panelId });
		return panel ? asPanelHandle(panel) : null;
	};

	const getPanelByType = (panelType: string): PanelHandle | null => {
		const panel = resolvePanelFromQuery({ panelType });
		return panel ? asPanelHandle(panel) : null;
	};

	const showPanel = (request: PanelSpawnRequest): PanelHandle | null => {
		const existingPanel = resolvePanelFromQuery({
			panelId: request.panelId,
			panelType: request.panelType
		});
		if (existingPanel) {
			if (request.title) {
				existingPanel.setTitle(request.title);
			}
			if (request.params) {
				existingPanel.api.updateParameters(request.params);
			}
			if (request.inactive !== true) {
				existingPanel.api.setActive();
			}
			return asPanelHandle(existingPanel);
		}

		const mergedRequest = withPanelDefaults(request);
		const createdPanelId = addPanel(mergedRequest);
		if (!createdPanelId) {
			return null;
		}

		const createdPanel = dockview?.getGroupPanel(createdPanelId);
		if (!createdPanel) {
			return null;
		}
		if (mergedRequest.inactive !== true) {
			createdPanel.api.setActive();
		}
		return asPanelHandle(createdPanel);
	};

	const panelController: PanelController = {
		accessPanel,
		getPanelById,
		getPanelByType,
		showPanel
	};

	const createGroupAddRenderer = (group: DockviewGroupPanel): IHeaderActionsRenderer => {
		const element = document.createElement('div');
		element.className = 'gc-group-panel-add-host';

		let mountedAction: MountedHeaderAction | null = mount(GroupPanelAddAction, {
			target: element,
			props: {
				availablePanels: availablePanelDefinitions,
				onAddPanel: (panelType: string): void => {
					addPanel(
						{ panelType },
						{
							referenceGroup: group,
							direction: 'within'
						}
					);
				}
			}
		}) as MountedHeaderAction;

		const dockviewDisposables: DockviewIDisposable[] = [];

		return {
			element,
			init: (parameters: IGroupHeaderProps): void => {
				dockviewDisposables.push(
					parameters.group.model.onDidActivePanelChange(() => {
						mountedAction?.closeMenu?.();
					})
				);
			},
			dispose: (): void => {
				mountedAction?.closeMenu?.();
				if (mountedAction) {
					void unmount(mountedAction);
					mountedAction = null;
				}
				for (const disposable of dockviewDisposables) {
					disposable.dispose();
				}
			}
		};
	};

	const createDefaultInitialPanels = (): PanelSpawnRequest[] => {
		if (!containerElement) {
			return [];
		}

		const rootFontSizePx =
			Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
		const remToPx = (value: number): number => Math.round(value * rootFontSizePx);

		return [
			{
				panelId: 'outliner',
				panelType: 'outliner',
				initialWidth: remToPx(10),
				maximumWidth: remToPx(30)
			},
			{
				panelId: 'dashboard',
				panelType: 'dashboard',
				position: {
					referencePanelId: 'outliner',
					direction: 'right'
				}
			},
			{
				panelId: 'logger',
				panelType: 'logger',
				initialHeight: remToPx(20),
				minimumHeight: remToPx(8),
				position: {
					referencePanelId: 'dashboard',
					direction: 'below'
				}
			},
			{
				panelId: 'warnings',
				panelType: 'warnings',
				position: {
					referencePanelId: 'logger',
					direction: 'within'
				},
				inactive: true
			},
			{
				panelId: 'inspector',
				panelType: 'inspector',
				initialWidth: remToPx(35),
				minimumWidth: remToPx(22),
				position: {
					referencePanelId: 'dashboard',
					direction: 'right'
				}
			}
		];
	};

	const applyLayoutFromContainer = (): boolean => {
		if (!dockview || !containerElement) {
			return false;
		}

		const width = containerElement.clientWidth;
		const height = containerElement.clientHeight;
		if (width <= 0 || height <= 0) {
			return false;
		}

		dockview.layout(width, height, true);
		return true;
	};

	onMount(() => {
		if (!containerElement) {
			return;
		}

		let isUnmounted = false;
		let persistLayoutTimer: ReturnType<typeof setTimeout> | null = null;
		const panelPersistenceDisposables = new Map<string, DockviewIDisposable>();
		const dockviewDisposables: DockviewIDisposable[] = [];

		const clearPersistLayoutTimer = (): void => {
			if (persistLayoutTimer === null) {
				return;
			}
			clearTimeout(persistLayoutTimer);
			persistLayoutTimer = null;
		};

		const persistLayoutNow = (): void => {
			if (!dockview || isUnmounted || isRestoringPersistedLayout) {
				return;
			}
			savePersistedDockLayout(dockview.toJSON());
		};

		const schedulePersistLayout = (): void => {
			if (!dockview || isUnmounted || isRestoringPersistedLayout) {
				return;
			}
			clearPersistLayoutTimer();
			persistLayoutTimer = setTimeout(() => {
				persistLayoutTimer = null;
				persistLayoutNow();
			}, 120);
		};

		const registerPanelPersistenceTracking = (panel: IDockviewPanel): void => {
			panelPersistenceDisposables.get(panel.id)?.dispose();

			const paramsDisposable = panel.api.onDidParametersChange(() => {
				schedulePersistLayout();
			});
			const titleDisposable = panel.api.onDidTitleChange(() => {
				schedulePersistLayout();
			});

			panelPersistenceDisposables.set(panel.id, {
				dispose: () => {
					paramsDisposable.dispose();
					titleDisposable.dispose();
				}
			});
		};

		const restoreDockLayout = (layout: SerializedDockview, source: string): boolean => {
			if (!dockview) {
				return false;
			}

			try {
				isRestoringPersistedLayout = true;
				dockview.fromJSON(layout);
				return true;
			} catch (error) {
				dockview.clear();
				console.warn(`[ui-persistence] Failed to restore dock layout from ${source}.`, error);
				return false;
			} finally {
				isRestoringPersistedLayout = false;
			}
		};

		const restorePersistedLayout = (): boolean => {
			const persistedLayout = loadPersistedDockLayout();
			if (!persistedLayout) {
				return false;
			}

			return restoreDockLayout(persistedLayout, 'storage');
		};

		dockview = new DockviewComponent(containerElement, {
			createComponent: createPanelRenderer,
			defaultTabComponent: 'golden-tab',
			createTabComponent: (options) =>
				options.name === 'golden-tab' ? createGoldenTabRenderer() : undefined,
			createLeftHeaderActionComponent: (group) => createGroupAddRenderer(group),
			theme: goldenDockviewTheme
		});
		appState.panels = panelController;

		const projectUiStateHostCleanup = registerProjectUiStateHost({
			captureDockLayout: () => {
				if (!dockview || isUnmounted) {
					return null;
				}
				clearPersistLayoutTimer();
				const layout = dockview.toJSON();
				savePersistedDockLayout(layout);
				return layout;
			},
			restoreDockLayout: (layout) => {
				if (!dockview || isUnmounted) {
					return false;
				}
				const restored = restoreDockLayout(layout, 'project file');
				if (restored) {
					schedulePersistLayout();
				}
				return restored;
			}
		});

		dockviewDisposables.push(
			dockview.onDidLayoutChange(() => {
				schedulePersistLayout();
			}),
			dockview.onDidActivePanelChange(() => {
				schedulePersistLayout();
			}),
			dockview.onDidAddPanel((panel) => {
				registerPanelPersistenceTracking(panel);
				schedulePersistLayout();
			}),
			dockview.onDidRemovePanel((panel) => {
				panelPersistenceDisposables.get(panel.id)?.dispose();
				panelPersistenceDisposables.delete(panel.id);
				schedulePersistLayout();
			})
		);

		const initializePanels = (): void => {
			if (isUnmounted || !dockview) {
				return;
			}

			const didRestorePersistedLayout = restorePersistedLayout();
			if (!didRestorePersistedLayout) {
				const initialPanelRequests = initialPanels ?? createDefaultInitialPanels();
				for (const panel of initialPanelRequests) {
					addPanel(panel);
				}
			} else {
				for (const panel of initialPanels?.filter(
					(request: PanelSpawnRequest) => request.required
				) ?? []) {
					if (!resolvePanelFromQuery({ panelType: panel.panelType })) {
						const referencePanelId = panel.position?.referencePanelId;
						const request =
							referencePanelId && !resolvePanelFromQuery({ panelId: referencePanelId })
								? { ...panel, position: undefined }
								: panel;
						addPanel(request);
					}
				}
			}

			for (const panel of dockview.panels) {
				registerPanelPersistenceTracking(panel);
			}

			void applyLayoutFromContainer();
			schedulePersistLayout();
		};

		let layoutRetryCount = 0;
		const maxLayoutRetries = 12;
		const initializeWhenSized = (): void => {
			if (isUnmounted) {
				return;
			}

			if (applyLayoutFromContainer() || layoutRetryCount >= maxLayoutRetries) {
				initializePanels();
				return;
			}

			layoutRetryCount += 1;
			requestAnimationFrame(initializeWhenSized);
		};

		initializeWhenSized();

		return () => {
			isUnmounted = true;
			clearPersistLayoutTimer();
			projectUiStateHostCleanup();
			for (const disposable of dockviewDisposables) {
				disposable.dispose();
			}
			for (const disposable of panelPersistenceDisposables.values()) {
				disposable.dispose();
			}
			panelPersistenceDisposables.clear();
			if (appState.panels === panelController) {
				appState.panels = null;
			}
			dockview?.dispose();
			dockview = undefined;
		};
	});
</script>

<div class="gc-content">
	<div bind:this={containerElement} class="gc-dockview"></div>
</div>
