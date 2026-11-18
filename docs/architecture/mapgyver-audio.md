# MapGyver audio output

In **File → Preferences → Audio**, enable output and choose **System Default** or a specific device. Each virtual output has a **Physical channel** setting. The default **Left** and **Right** map to the first two physical channels. Set a virtual output to **Unassigned** to leave it silent.

In **Inspector → Channels → Audio** on a Video, choose a virtual output for each side of the decoded stereo signal and set **Volume** in dB. The channel starts at 0 dB with Left and Right assigned. Disabling its Audio folder mutes that channel while the Video transport continues. Different Video channels can use different files, virtual outputs and volumes.

MapGyver stores the selected device and virtual outputs as typed Preferences in app data. Project files can refer to the virtual output's saved identity. Renaming, remapping, or reloading a virtual output preserves that identity. Duplicating it gives the copy its own identity. If a device or physical channel is unavailable, its authored key stays selected and its route remains silent until the key can be resolved. An explicit device selection waits for that device. **System Default** follows the operating system's current default.

The shared Golden desktop host starts the audio service once in desktop and headless sessions. MapGyver projects Preferences into the public Golden Audio configuration on a worker. Golden Audio owns device discovery, stream negotiation, recovery and the render callback. The current service uses Golden Audio's standard 48 kHz engine clock. Its output-control inspection reports stream and route state.

Golden Video decodes and seeks embedded audio as timestamped stereo PCM at the engine rate. A MapGyver audio worker projects the shared Video transport onto Golden Audio's output clock and applies per-channel gain and routing. Golden Audio accepts fixed-size blocks tagged with the render frame and routing generation. Its render cursor supplies the monotonic clock anchor and queued device-bridge frames. Pause, seek and project replacement invalidate pending blocks. MapGyver projects each bus lane onto a virtual output before the physical patch.

The [M16/1 gate](../plan/MapGyver_Implementation_Plan.md#earlier-sgf-feedback-m1616) tracks the remaining signal and device-recovery qualification.

## Matrix Cells

Right-click a Cell and choose **Inspect Cell settings**, then open **Audio**. **Source → Media** uses the selected Video channel's audio and its routing. **Source → File** uses the Cell's own file, virtual outputs and dB volume. Use this mode on an empty Cell for audio-only playback. An unavailable file stays assigned, and the worker retries it at the Cell's current time.

Each file starts with the Cell launch and plays once. The source follows the same per-Layer Matrix transition envelope as its picture. Multiple Cells that inherit the same Video channel share the source and combine their weights up to unity. Muting a Cell, Layer or Column removes its contribution. Stopping retires its audio, and a catalog Video started separately still follows its own transport.

Matrix projects committed Cell choices and transition envelopes into the existing MapGyver PCM worker. Local files reuse the Video audio reader and share the mixer's voice ceiling. Video and the audio device retain their existing clock and ownership boundaries. Projects store the typed Cell settings. Launches and active mixes are transient.
