<script lang="ts">
	let { nodes } = $props();

	function syntaxHighlight(json: string) {
		json = json.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
		return json.replace(
			/("(\\u[a-zA-Z0-9]{4}|\\[^u]|[^\\"])*"(\s*:)?|\b(true|false|null)\b|-?\d+(?:\.\d*)?(?:[eE][+\-]?\d+)?)/g,
			function (match) {
				var cls = 'number';
				if (/^"/.test(match)) {
					if (/:$/.test(match)) {
						cls = 'key';
					} else {
						cls = 'string';
					}
				} else if (/true|false/.test(match)) {
					cls = 'boolean';
				} else if (/null/.test(match)) {
					cls = 'null';
				}
				return '<span class="' + cls + '">' + match + '</span>';
			}
		);
	}
</script>

{#if nodes.length > 0}
	<div class="json-container">
		<pre>{@html syntaxHighlight(JSON.stringify(nodes[0], null, 2))}</pre>
	</div>
{/if}

<style>
	.json-container {
		font-family: monospace;
		font-size: 0.7rem;
		color: var(--text-color);
		user-select: text;
	}

	:global {
		.key {
			color: var(--color-key, #d14);
		}
		.string {
			color: var(--color-string, #1a1);
		}
		.number {
			color: var(--color-number, #08f);
		}
		.boolean {
			color: var(--color-boolean, #f50);
		}
		.null {
			color: var(--color-null, #888);
		}
	}
</style>
