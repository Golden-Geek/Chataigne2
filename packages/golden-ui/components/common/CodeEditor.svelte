<script lang="ts">
	import { historyField } from '@codemirror/commands';
	import { Compartment, EditorState, type Extension } from '@codemirror/state';
	import { javascript } from '@codemirror/lang-javascript';
	import { css } from '@codemirror/lang-css';
	import { oneDark } from '@codemirror/theme-one-dark';
	import { EditorView, placeholder as codeMirrorPlaceholder } from '@codemirror/view';
	import { basicSetup } from 'codemirror';

	type CodeLanguage = 'javascript' | 'css' | 'plain';

	interface PersistedEditorSession {
		json: unknown;
		doc: string;
		scrollTop: number;
		scrollLeft: number;
		hadFocus: boolean;
	}

	const PERSISTED_EDITOR_STATE_LIMIT = 64;
	const persistedEditorSessions = new Map<string, PersistedEditorSession>();

	const normalizePersistKey = (value: string): string => value.trim();

	const persistEditorSession = (key: string, session: PersistedEditorSession): void => {
		if (persistedEditorSessions.has(key)) {
			persistedEditorSessions.delete(key);
		}
		persistedEditorSessions.set(key, session);
		if (persistedEditorSessions.size <= PERSISTED_EDITOR_STATE_LIMIT) {
			return;
		}
		const oldestKey = persistedEditorSessions.keys().next().value;
		if (typeof oldestKey === 'string') {
			persistedEditorSessions.delete(oldestKey);
		}
	};

	let {
		value = '',
		language = 'plain',
		readonly = false,
		placeholder = '',
		minHeight = '12rem',
		fill = false,
		persistKey = '',
		commitOnBlur = false,
		onchange = undefined,
		oncommit = undefined,
		onfocus = undefined,
		onblur = undefined
	} = $props<{
		value?: string;
		language?: CodeLanguage;
		readonly?: boolean;
		placeholder?: string;
		minHeight?: string;
		fill?: boolean;
		persistKey?: string;
		commitOnBlur?: boolean;
		onchange?: ((nextValue: string) => void) | undefined;
		oncommit?: ((nextValue: string) => void | Promise<void>) | undefined;
		onfocus?: (() => void) | undefined;
		onblur?: (() => void) | undefined;
	}>();

	let hostElement = $state<HTMLDivElement | null>(null);
	let editorView: EditorView | null = null;
	let applyingExternalValue = false;

	const languageCompartment = new Compartment();
	const editableCompartment = new Compartment();
	const placeholderCompartment = new Compartment();

	const resolveLanguageExtension = (selected: CodeLanguage): Extension => {
		if (selected === 'javascript') {
			return javascript();
		}
		if (selected === 'css') {
			return css();
		}
		return [];
	};

	const resolvePlaceholderExtension = (text: string): Extension => {
		const normalized = text.trim();
		if (normalized.length === 0) {
			return [];
		}
		return codeMirrorPlaceholder(normalized);
	};

	const saveShortcutTriggered = (event: KeyboardEvent): boolean =>
		!readonly && (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's';

	const commitCurrentValue = (options: { keepFocus?: boolean } = {}): void => {
		const view = editorView;
		if (!view) {
			return;
		}
		const commitResult = oncommit?.(view.state.doc.toString());
		if (!options.keepFocus) {
			return;
		}

		const refocusIfNeeded = (): void => {
			if (editorView !== view || view.hasFocus) {
				return;
			}
			view.focus();
		};

		queueMicrotask(refocusIfNeeded);
		void Promise.resolve(commitResult).finally(() => {
			queueMicrotask(refocusIfNeeded);
		});
	};

	const interactionHandlers = EditorView.domEventHandlers({
		focus: () => {
			onfocus?.();
			return false;
		},
		keydown: (event) => {
			if (saveShortcutTriggered(event)) {
				event.preventDefault();
				event.stopPropagation();
				event.stopImmediatePropagation();
				commitCurrentValue({ keepFocus: true });
				return true;
			}
			return false;
		},
		blur: () => {
			if (commitOnBlur && !readonly) {
				commitCurrentValue();
			}
			onblur?.();
			return false;
		}
	});

	const changeListener = EditorView.updateListener.of((update) => {
		if (!update.docChanged || applyingExternalValue) {
			return;
		}
		const nextValue = update.state.doc.toString();
		if (nextValue === value) {
			return;
		}
		onchange?.(nextValue);
	});

	const buildExtensions = (): Extension[] => [
		basicSetup,
		oneDark,
		interactionHandlers,
		changeListener,
		EditorView.lineWrapping,
		EditorView.theme({
			'&': {
				height: '100%',
				minHeight: 'var(--code-editor-min-height)'
			},
			'.cm-scroller': {
				fontFamily: "'Cascadia Code', 'Consolas', monospace",
				fontSize: '0.72rem',
				lineHeight: '1.35'
			}
		}),
		languageCompartment.of(resolveLanguageExtension(language)),
		editableCompartment.of(EditorView.editable.of(!readonly)),
		placeholderCompartment.of(resolvePlaceholderExtension(placeholder))
	];

	const restorePersistedViewState = (
		view: EditorView,
		session: PersistedEditorSession | undefined
	): void => {
		if (!session) {
			return;
		}
		const apply = (): void => {
			view.scrollDOM.scrollTop = session.scrollTop;
			view.scrollDOM.scrollLeft = session.scrollLeft;
			if (session.hadFocus) {
				view.focus();
			}
		};
		queueMicrotask(apply);
		requestAnimationFrame(apply);
	};

	const buildEditor = (host: HTMLDivElement): EditorView => {
		const normalizedKey = normalizePersistKey(persistKey);
		const persistedSession =
			normalizedKey.length > 0 ? persistedEditorSessions.get(normalizedKey) : undefined;
		const extensions = buildExtensions();

		let state: EditorState;
		if (persistedSession && persistedSession.doc === value) {
			try {
				state = EditorState.fromJSON(
					persistedSession.json as object,
					{ extensions },
					{ history: historyField }
				);
			} catch (_error) {
				state = EditorState.create({
					doc: value,
					extensions
				});
			}
		} else {
			state = EditorState.create({
				doc: value,
				extensions
			});
		}

		const view = new EditorView({ state, parent: host });
		restorePersistedViewState(view, persistedSession);
		return view;
	};

	const captureEditorSession = (view: EditorView): void => {
		const normalizedKey = normalizePersistKey(persistKey);
		if (normalizedKey.length === 0) {
			return;
		}
		persistEditorSession(normalizedKey, {
			json: view.state.toJSON({ history: historyField }),
			doc: view.state.doc.toString(),
			scrollTop: view.scrollDOM.scrollTop,
			scrollLeft: view.scrollDOM.scrollLeft,
			hadFocus: view.hasFocus
		});
	};

	$effect(() => {
		const host = hostElement;
		if (!host || editorView) {
			return;
		}

		const view = buildEditor(host);
		editorView = view;
		return () => {
			captureEditorSession(view);
			view.destroy();
			if (editorView === view) {
				editorView = null;
			}
		};
	});

	$effect(() => {
		const view = editorView;
		if (!view) {
			return;
		}
		const currentValue = view.state.doc.toString();
		if (currentValue === value) {
			return;
		}
		const hadFocus = view.hasFocus;
		const preservedSelection = view.state.selection;
		const preservedScrollTop = view.scrollDOM.scrollTop;
		const preservedScrollLeft = view.scrollDOM.scrollLeft;
		applyingExternalValue = true;
		try {
			view.dispatch({
				changes: {
					from: 0,
					to: view.state.doc.length,
					insert: value
				},
				selection: preservedSelection
			});
			if (hadFocus) {
				view.focus();
			}
			view.scrollDOM.scrollTop = preservedScrollTop;
			view.scrollDOM.scrollLeft = preservedScrollLeft;
		} finally {
			applyingExternalValue = false;
		}
	});

	$effect(() => {
		const view = editorView;
		if (!view) {
			return;
		}
		view.dispatch({
			effects: languageCompartment.reconfigure(resolveLanguageExtension(language))
		});
	});

	$effect(() => {
		const view = editorView;
		if (!view) {
			return;
		}
		view.dispatch({
			effects: editableCompartment.reconfigure(EditorView.editable.of(!readonly))
		});
	});

	$effect(() => {
		const view = editorView;
		if (!view) {
			return;
		}
		view.dispatch({
			effects: placeholderCompartment.reconfigure(resolvePlaceholderExtension(placeholder))
		});
	});
</script>

<div
	class="code-editor"
	class:fill
	style={`--code-editor-min-height:${minHeight};`}
	bind:this={hostElement}>
</div>

<style>
	.code-editor {
		width: 100%;
		min-height: var(--code-editor-min-height);
		border: solid 0.06rem rgba(255, 255, 255, 0.16);
		border-radius: 0.25rem;
		overflow: hidden;
		background-color: var(--bg-color);
	}

	.code-editor.fill {
		height: 100%;
		min-height: 0;
	}

	.code-editor :global(.cm-editor) {
		height: 100%;
		min-height: var(--code-editor-min-height);
	}

	.code-editor.fill :global(.cm-editor) {
		min-height: 0;
	}

	.code-editor :global(.cm-scroller) {
		overflow: auto;
	}

	.code-editor :global(.cm-content),
	.code-editor :global(.cm-gutter) {
		padding-block: 0.35rem;
	}
</style>
