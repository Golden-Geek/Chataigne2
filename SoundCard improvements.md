The SoundCard module is waaay to complex for users right now. Let's redesign it to simplify and optimize :

- First, it should not "connect" to all devices on creation. for instance, right now, even when i don't want to use ASIO, i see ASIO driver being started by Chataigne when i create the sound card, and i can't even choose some drivers because they're used by the running ASIO4ALL because of Chataigne itself. The way it should go is :

- in "Connection" section of the module, remove the available interfaces extra informations
  - first parameter is "Audio Driver" : selection between different drivers (e.g. on windows : WASAPI, ASIO or "None"). This is effective for both input and output.
  - Then, 2 parameters "Input Device" and "Output Device" to choose which device to connect to for this driver. list is updated when changing driver. Also have a "None" option. Input is None by default, Output is system default by default.
  - Chataigne ONLY connects to the exact device and driver it's selected for, NOT the other ones.
  - Next parameters are

    - Sample Rate and Buffer Size : those are enum that are updated when an input or output is connected (or both) with available configurations that are available for the connected devices. If input or output is set to None, then list is updated to match available configurations from the used one obviously.
  - There is a difference between "user set" configuration and "system updated" configuration. If a user sets a configuration manually from the UI, and then a device is unplugged, there should be a smart recovery system. Fallback is to stop sound, not set a default driver and device instead. As long as user doesn't set anything themselves manually, the module should be aware of changes and recover the last "user set" configuration when it becomes available again.

    - Virtual Channel Routing : I don't want to show users the term "virtual", it brings complexity in the mind. I want users to have a clear interface of what they're using and patching. If "Input device" is used (i.e. not set to "None"), then an Input Routing container is shown. in this container, there is a "Input Channels" int parameter to choose how many virtual channels to have, and then a patch-like UI in the style of "routing.png" : on the left is device channels (with their names), and on the right is virtual channels, that are editable string fields (default named "Input 1","Input 2", etc.). Default routing is parallel stereo on first 2 channels. Output is the same, except virtual channels are on the left and device channels on the right.

Forget about "Device Profiles", this is way to complex and users shouldn't have to deal with that

- Now in Parameters section :
  - if input is used (otherwise hidden):

    - Master Input Volume
    - Channel Volumes container > one float for each channel
  - same for output
  - "Processing" section inside parameters

    - Pitch Detection, disabled by default. If enabled, a Pitch detection appears in Values. If disabled, NO pitch detection container values.
    - Spectral Analysis, disabled by default. same behaviour as pitch detection for showing/creating/removing container in values. Spectral analysis should not function like that. For now, we'll leave it like that but we'll come back to it later.

- Values :
  - Input (if used) :

    - Master Input Volume
    - Channels
      - Per Channel Input Volume
  - Output (if used) :

    - Master Output Volume
    - Channels
      - Per Channel Output Volume
  - Pitch Detection (if enabled)
  - Spectral Analysis (if enabled)


