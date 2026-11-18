# MapGyver audio output

In **File → Preferences → Audio**, enable output and choose **System Default** or a specific device. Each virtual output has a **Physical channel** setting. The default **Left** and **Right** map to the first two physical channels. Set a virtual output to **Unassigned** to leave it silent.

MapGyver stores the selected device and virtual outputs as typed Preferences in app data. Project files can refer to the virtual output's saved identity. Renaming, remapping, or reloading a virtual output preserves that identity. Duplicating it gives the copy its own identity. If a device or physical channel is unavailable, its authored key stays selected and its route remains silent until the key can be resolved. An explicit device selection waits for that device. **System Default** follows the operating system's current default.

The shared Golden desktop host starts the audio service once in desktop and headless sessions. MapGyver projects Preferences into the public Golden Audio configuration on a worker. Golden Audio owns device discovery, stream negotiation, recovery and the render callback. The current service uses Golden Audio's standard 48 kHz engine clock. Its output-control inspection reports stream and route state.

Video sample delivery, per-channel gain and native signal verification are the remaining work in [M16/1](../plan/MapGyver_Implementation_Plan.md#earlier-sgf-feedback-m1616).
