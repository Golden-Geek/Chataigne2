/*
 * App Control module functions (call them on local):
 *   local.launchWatchedApp(target, arguments = "", workingDirectory = "")
 *   local.launchApp(executablePath, arguments = "", workingDirectory = "")
 *   local.launchCommandLine(commandLine, workingDirectory = "")
 *   local.killProcess(target, matchMode = "exact", hardKill = false, watched = false)
 *   local.controlWindow(target, action, matchMode = "exact", watched = false, x = 0, y = 0, width = 1280, height = 720, alwaysOnTop = true)
 *
 * `target` matches a watched app label/path when `watched = true`; otherwise it matches
 * a process name or executable path using the selected match mode.
 * `action` accepts "move", "resize", "bounds", "minimize", "maximize", "restore",
 * "tray", "show", and "always_on_top".
 */