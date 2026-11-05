import type { DockviewIDisposable, ITabRenderer, TabPartInitParameters } from 'dockview-core';
import { PanelContentBindings, type PanelContentBinding } from './panel-content-binding';
import './panel-content-binding.css';

export const createGoldenTabRenderer = (
	iconForType: (type: string) => string | undefined = () => undefined,
	bindings = new PanelContentBindings()
): ITabRenderer => {
	const element = document.createElement('div');
	element.className = 'gc-dv-tab';

	const label = document.createElement('span');
	label.className = 'gc-dv-tab-label';

	const closeButton = document.createElement('button');
	closeButton.type = 'button';
	closeButton.className = 'gc-dv-tab-close';
	closeButton.setAttribute('aria-label', 'Close panel');
	closeButton.setAttribute('title', 'Close');

	const lockButton = document.createElement('button');
	lockButton.type = 'button';
	lockButton.className = 'gc-dv-tab-binding';
	lockButton.hidden = true;
	element.append(label, lockButton, closeButton);

	let disposables: DockviewIDisposable[] = [];

	return {
		element,
		init: (parameters: TabPartInitParameters): void => {
			let binding: PanelContentBinding | null = null;
			const unsubscribe = bindings.subscribe(parameters.api.id, (value) => {
				binding = value;
				lockButton.hidden = !value;
				lockButton.disabled = !value || (!value.locked && !value.canLock);
				const action = value?.locked ? 'Unlock panel content' : 'Lock panel content';
				lockButton.setAttribute('aria-label', action);
				lockButton.title = action;
				lockButton.setAttribute('aria-pressed', String(value?.locked ?? false));
			});
			label.textContent = parameters.title;
			element.dataset.contentPanelId = parameters.api.id;
			element.dataset.panelType = parameters.api.component;
			const source = iconForType(parameters.api.component);
			if (source) {
				const icon = document.createElement('img');
				icon.className = 'gc-dv-tab-icon';
				icon.src = source;
				icon.alt = '';
				icon.draggable = false;
				Object.assign(icon.style, {
					width: '1em',
					height: '1em',
					objectFit: 'contain',
					flex: '0 0 auto'
				});
				element.insertBefore(icon, label);
			}

			const onTitleChange = parameters.api.onDidTitleChange((event) => {
				label.textContent = event.title;
			});

			const onClick = (event: MouseEvent): void => {
				event.preventDefault();
				event.stopPropagation();
				parameters.api.close();
			};

			const stopMouseDown = (event: MouseEvent): void => {
				event.preventDefault();
				event.stopPropagation();
			};

			closeButton.addEventListener('click', onClick);
			closeButton.addEventListener('mousedown', stopMouseDown);
			const toggle = (event: MouseEvent): void => {
				event.preventDefault();
				event.stopPropagation();
				binding?.toggle();
			};
			lockButton.addEventListener('click', toggle);
			lockButton.addEventListener('mousedown', stopMouseDown);

			disposables = [
				onTitleChange,
				{
					dispose: () => {
						unsubscribe();
						lockButton.removeEventListener('click', toggle);
						lockButton.removeEventListener('mousedown', stopMouseDown);
						closeButton.removeEventListener('click', onClick);
						closeButton.removeEventListener('mousedown', stopMouseDown);
					}
				}
			];
		},
		dispose: (): void => {
			for (const disposable of disposables) {
				disposable.dispose();
			}

			disposables = [];
		}
	};
};
