# Video intro and loop

Enable **Loop** on a Video and set **Loop Start** to the source time where repeats should begin. Playback starts at the cropped **Start**, plays through **End**, then repeats from **Loop Start**. The value `0` follows the crop start. A marker before the crop follows the crop start as well. A crop or file change that puts the marker at or after the effective end makes playback finite until the repeat section becomes available again. The marker stays authored through the change.

**Auto-Smooth Loop** overlaps the tail with the beginning of the repeat section. The overlap is limited to half of the shortest available repeat section across the Video channels. After a smooth handoff, playback resumes after the part of the incoming section already blended. Reverse playback can traverse the full crop once, then repeats the same tail in reverse. Pause and resume preserve the current repeat. Stop returns to the crop start, and a seek uses the crop-relative position shown in the Inspector.

MapGyver stores **Loop Start** as a typed media parameter and uses the same edit through the Inspector, `Set Video Loop Start` command and `local.setVideoLoopStart`. One Video clock projects the intro and repeated source coordinates into picture windows and audio samples. Golden Video supplies the shared loop math, and Golden Audio consumes the scheduled PCM.

## Separate files

Enable **Loop** on the Media. Under **Inspector → Channels → [channel name]**, set **File** to the intro and **Loop File** to the clip that repeats. In this mode, **Start** and **End** trim the intro. The loop file starts at its first frame, regardless of **Loop Start**. **Current Position** includes both clips. With several Video channels, the longest intro and longest repeat define the shared timeline. A shorter channel holds its last frame. A channel without a loop file holds its last intro frame during the repeat.

The intro cuts to the loop file. **Auto-Smooth Loop** blends the end of the loop file into its beginning on subsequent wraps and skips the already blended part on each restart. It also applies at the directional wrap when playing in reverse. If an authored loop file is initially unavailable, the intro is playable and holds at its end. Playback resumes into the repeat once its duration is known if Play is still requested. A repeat file that disappears after it was loaded keeps its known time span so other channels can stay on the shared clock.

Enable **Audio** in that same channel for audio routing. **Loop Audio File** replaces the embedded audio from the start of the repeat section and uses the existing channel outputs and volume. This works with either **Loop Start** or a separate **Loop File**. A short audio file is silent for the remainder of the repeat. With **Loop Audio File** empty, embedded audio follows whichever picture file is playing.

The channel's embedded subtitles come from the intro file. A local subtitle file uses source-time coordinates across the full sequence, including the appended repeat section.

The typed channel parameters use the existing Inspector, project edits and persistence. The metadata worker probes both clips without render demand. The file worker converts the separate sequence to the intro dimensions so consumers keep one stable texture and both images can share a crossfade. Each decoder retains its existing admission charge. If the overlap decoder cannot acquire storage, playback keeps the primary picture and retries admission. The single-file path continues to use its existing native codec tiers.

[M16/2](../plan/MapGyver_Implementation_Plan.md#earlier-sgf-feedback-m1616) owns the feature checks. Hardware performance qualification is tracked with the broader Video gates.
