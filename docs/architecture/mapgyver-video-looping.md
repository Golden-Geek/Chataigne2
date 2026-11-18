# Video intro and loop

Enable **Loop** on a Video and set **Loop Start** to the source time where repeats should begin. Playback starts at the cropped **Start**, plays through **End**, then repeats from **Loop Start**. The value `0` follows the crop start. A marker before the crop follows the crop start as well. A crop or file change that puts the marker at or after the effective end makes playback finite until the repeat section becomes available again. The marker stays authored through the change.

**Auto-Smooth Loop** overlaps the tail with the beginning of the repeat section. The overlap is limited to half of the shortest available repeat section across the Video channels. After a smooth handoff, playback resumes after the part of the incoming section already blended. Reverse playback can traverse the full crop once, then repeats the same tail in reverse. Pause and resume preserve the current repeat. Stop returns to the crop start, and a seek uses the crop-relative position shown in the Inspector.

MapGyver stores **Loop Start** as a typed media parameter and uses the same edit through the Inspector, `Set Video Loop Start` command and `local.setVideoLoopStart`. One Video clock projects the intro and repeated source coordinates into picture windows and audio samples. Golden Video supplies the shared loop math, and Golden Audio consumes the scheduled PCM.

[M16/2](../plan/MapGyver_Implementation_Plan.md#earlier-sgf-feedback-m1616) also covers a separate loop Video and optional independent loop audio.
