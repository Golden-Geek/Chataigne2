# Paging System Architecture

This document defines the unified, project-wide paging architecture for Chataigne 2. It establishes how physical or virtual interface surfaces interact with the app-wide hierarchical state tree while maintaining strict structural modularity.

## 1. Core Principles

1. **Modular Structural Ownership:** Modules completely own their local node sub-tree layout. A module defines the exact properties, structure, and shapes of its local interactive nodes.
2. **Predictable Address Space:** Pages are explicit structural folders within the module tree (`.../pages/page_<name>/`). Addresses never mutate dynamically or swap pointers at runtime. This guarantees that links, expressions, dashboard elements, and scripts have stable, immutable targets.
3. **Module-Local Structure, Global Orchestration:** Pages live strictly inside the module that handles their physical realization. Project-wide synchronous paging is achieved purely by driving a module’s standardized `active_page` primitive parameter via the app-wide Preset/State system.
4. **Selective Interface Capabilities:** Not all modules are relevant to paging. Paging infrastructure is opted into by implementing a dedicated capability interface, marking a module explicitly as a **Pageable Module**.

---

## 2. Architectural Components

```
[ Modules / StreamDeck (A Pageable Module) ]
    ├── parameters/
    │     └── active_page: String = "lighting"  <-- Orchestrated by Global Presets
    │
    ├── permanent/
    │     └── keys/
    │           └── key_0 (e.g., Master Stop)  <-- Fixed address link
    │
    └── pages/
          ├── page_audio/
          │     └── keys/
          │           ├── key_0 (Vol Up)        <-- Address: .../pages/page_audio/keys/key_0
          │           └── key_1 (Vol Down)      <-- Address: .../pages/page_audio/keys/key_1
          │
          └── page_lighting/
                └── keys/
                      ├── key_0 (Strobe Cue)    <-- Address: .../pages/page_lighting/keys/key_0
                      └── key_1 (Wash Color)    <-- Address: .../pages/page_lighting/keys/key_1

```

### 2.1 Interactivity Elements (The Control Shapes)

Instead of forcing modules to use arbitrary, hardcoded control definitions (such as standard buttons or faders), each module defines its own native **Control Shapes**. These shapes are structured nodes containing two explicit streams of data primitives:

* **Feedback Primitives (Outbound/Write):** Properties updating the physical/visual hardware state (e.g., `text`, `background_color`, `image_path`, `led_ring_mode`).
* **Activity Primitives (Inbound/Read-Only):** Properties capturing active user manipulation (e.g., `is_pressed`, `delta`, `absolute_position`).

Modules can choose to implement unique custom shapes, or reuse common templates shared across the ecosystem when hardware capabilities overlap (e.g., an infinite encoder with an LED ring).

### 2.2 The Viewport Router (Hardware Mapping)

The physical hardware loop acts as a sliding **Viewport Window**. It maintains a static configuration map matching its physical layout indices (e.g., Grid Key 0 to 15).

Each physical key or input index contains a local user property:

* `is_paged`: Boolean.
* `permanent_target_name`: String (Used only if `is_paged == false`).

At runtime, the driver calculates its execution path purely by text concatenation based on the value of the local `active_page` parameter:

* If `is_paged` is **false**: Viewport binds directly to `.../permanent/<permanent_target_name>`.
* If `is_paged` is **true**: Viewport binds to `.../pages/page_<active_page>/<control_shape_folder>/<index>`.

---

## 3. Engineering Implementation Spec (`golden_core`)

### 3.1 Defining Control Element Declarations

To allow modules to define arbitrary hardware shapes cleanly, `golden_core` provides the foundational primitive layout definition structs:

```rust
pub enum InterfacePrimitiveKind {
    Bool,
    Float,
    String,
    Color,
    FilePath,
}

pub struct ElementFieldDecl {
    pub name: String,
    pub primitive_kind: InterfacePrimitiveKind,
    pub read_only: bool,
}

pub struct CustomShapeDefinition {
    pub shape_id: String,
    pub friendly_label: String,
    pub feedback_fields: Vec<ElementFieldDecl>,
    pub activity_fields: Vec<ElementFieldDecl>,
}

```

### 3.2 The Pageable Module Interface

Modules opt into the paging framework by exposing their structural layout template through a dedicated trait contract. If a module does not implement this trait, it is ignored by the paging generation sub-systems.

```rust
pub struct LayoutTemplateItem {
    pub shape_id: String,
    pub structural_folder: String,
    pub element_name: String,
    pub hardware_slot_index: usize,
}

pub trait PageableModuleCapability {
    /// Declares the custom shapes this module introduces
    fn declared_shapes(&self) -> Vec<CustomShapeDefinition>;
    
    /// Dictates the structure automatically spawned inside every new page folder
    fn page_layout_template(&self) -> Vec<LayoutTemplateItem>;
    
    /// Dictates permanent nodes that sit outside the paging sub-tree
    fn permanent_layout_template(&self) -> Vec<LayoutTemplateItem>;
}

```

---

## 4. Runtime & Lifecycle Mechanics (`Chataigne2`)

### 4.1 Automated Page Node Generation

When a module implementing `PageableModuleCapability` is added to the project, the core framework automatically hooks into its creation lifecycle:

1. It inserts the standard structural parameter `/parameters/active_page` (String).
2. It generates the `/permanent/` root container and seeds it using the module's `permanent_layout_template`.
3. It seeds an initial `/pages/page_default/` directory using the module's `page_layout_template`.

When a user triggers a `CreatePage(name)` request via the UI or a script, the engine instantiates the exact folder nodes required by the template:

```rust
pub fn generate_page_nodes(
    ctx: &mut ProcessCtx,
    module_id: NodeId,
    capability: &impl PageableModuleCapability,
    page_name: &str,
) {
    let sanitized_page_name = format!("page_{}", page_name.to_lowercase().replace(' ', "_"));
    
    // Allocate parent path: modules/<module_name>/pages/<sanitized_page_name>/
    for item in capability.page_layout_template() {
        // Instantiate the component primitives under the target path:
        // modules/<module_name>/pages/<sanitized_page_name>/<structural_folder>/<element_name>
    }
}

```

### 4.2 Hardware Synchronization Loop

The module’s processing loop remains completely decoupled from paging logic. It behaves as a stateless viewport looking at the computed path strings:

```rust
pub fn update_hardware_view(&mut self, ctx: &mut ProcessCtx) {
    let current_page = self.active_page_param.get().to_lowercase();

    for physical_input in &self.hardware_inputs {
        let absolute_target_path = if !physical_input.is_paged {
            format!("{}/permanent/{}", self.module_base_path, physical_input.name)
        } else {
            format!("{}/pages/page_{}/{}", self.module_base_path, current_page, physical_input.name)
        };

        // 1. Resolve absolute parameters via tree address paths
        // 2. Read Outbound Feedback primitives (text, colors) -> Push to Device
    }
}

pub fn handle_incoming_hardware_event(&mut self, ctx: &mut ProcessCtx, hardware_index: usize, value: f32) {
    let current_page = self.active_page_param.get().to_lowercase();
    let physical_input = &self.hardware_inputs[hardware_index];

    let absolute_activity_path = if !physical_input.is_paged {
        format!("{}/permanent/{}/value", self.module_base_path, physical_input.name)
    } else {
        format!("{}/pages/page_{}/{}/value", self.module_base_path, current_page, physical_input.name)
    };

    // Safely pipe the raw value primitive straight into the resolved target address
    ctx.update_parameter_by_address(&absolute_activity_path, ParamValue::Float(value));
}

```

---

## 5. UI Architecture Implementation (`src-ui`)

Using Svelte 5's reactive runes (`$state`, `$derived`), the frontend inspector renders the hardware layout dynamically by observing the active structural layer.

### 5.1 Inspector Layout Resolution

The inspector checks for paging capability metadata flags generated at the protocol boundary. If the module is a **Pageable Module**, it embeds the universal page management header bar:

```html
<script lang="ts">
    import { getContext } from 'svelte';
    let { moduleNode } = $props(); // Exposed via the standard module container
    
    // Live derived reflections targeting the explicit active page tree partition
    let activePage = $derived(moduleNode.parameters.active_page);
    let activePageFolder = $derived(moduleNode.children.pages[`page_${activePage.toLowerCase()}`]);
</script>

<div class="pageable-module-container">
    <header class="page-orchestration-bar">
        <label>Active Local Viewport:</label>
        <select bind:value={moduleNode.parameters.active_page}>
            {#each Object.keys(moduleNode.children.pages) as pageKey}
                <option value={pageKey.replace('page_', '')}>{pageKey}</option>
            {/each}
        </select>
        <button onclick={() => executeAddPageCommand(moduleNode.id)}>+ New Page</button>
    </header>

    <main class="viewport-grid">
        <ControlSection folder={moduleNode.children.permanent} label="Permanent Layout" />
        
        {#if activePageFolder}
            <ControlSection folder={activePageFolder} label="Page: {activePage}" />
        {:else}
            <div class="fallback-warning">Page "{activePage}" is not structurally initialized.</div>
        {/if}
    </main>
</div>

```

---

## 6. Project-Wide Integration & Workflows

### 6.1 Seamless Presets Integration

Because pages are native paths containing standard primitive parameters, Chataigne 2's app-wide **Preset System** coordinates project-wide page switches effortlessly with zero custom scripts or workflow silos:

* **Synchronous Page Flips:** A global preset named `State_Performance_Start` can include the following parameter maps:
* `/modules/streamdeck/parameters/active_page` = `"main_mix"`
* `/modules/loupedeck/parameters/active_page` = `"eq_focus"`
* `/modules/midifighter/parameters/active_page` = `"track_triggers"`


* **Asymmetric Layering:** Because each module operates its own local parameter state machine, an operator can fire a cue that switches the Stream Deck pages while leaving the MIDI fader wings locked to their active audio mix context.
* **Partial Overrides:** A momentary state preset can instantly overwrite a module's `active_page` parameter value, snapping physical viewports to an emergency or shift layout layer, and immediately return to the previous configuration when released.