import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";

function readArguments(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith("--") || value === undefined) {
      throw new Error(`invalid argument list near '${name ?? "<end>"}'`);
    }
    values.set(name.slice(2), value);
  }
  return values;
}

function requiredArgument(argumentsMap, name) {
  const value = argumentsMap.get(name);
  if (!value) {
    throw new Error(`missing required --${name} argument`);
  }
  return value;
}

async function launchBrowser(chromium) {
  const configuredExecutable = process.env.GC_UI_BROWSER_EXECUTABLE?.trim();
  const configuredChannel = process.env.GC_UI_BROWSER_CHANNEL?.trim();
  if (configuredExecutable) {
    return chromium.launch({ headless: true, executablePath: configuredExecutable });
  }
  if (configuredChannel) {
    return chromium.launch({ headless: true, channel: configuredChannel });
  }

  const attempts = process.platform === "win32" ? [{ channel: "msedge" }, {}] : [{ channel: "chrome" }, {}];
  const errors = [];
  for (const attempt of attempts) {
    try {
      return await chromium.launch({ headless: true, ...attempt });
    } catch (error) {
      errors.push(error instanceof Error ? error.message : String(error));
    }
  }
  throw new Error(`no usable Chromium browser was found:\n${errors.join("\n")}`);
}

const argumentsMap = readArguments(process.argv.slice(2));
const repositoryRoot = path.resolve(requiredArgument(argumentsMap, "repository-root"));
const url = requiredArgument(argumentsMap, "url");
const screenshotPath = path.resolve(requiredArgument(argumentsMap, "screenshot"));
const timeoutMs = Number.parseInt(argumentsMap.get("timeout-ms") ?? "60000", 10);
if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
  throw new Error("--timeout-ms must be a positive integer");
}

const requireFromUi = createRequire(path.join(repositoryRoot, "src-ui", "package.json"));
const { chromium } = requireFromUi("playwright-core");
const result = {
  url,
  websocketCount: 0,
  receivedFrames: 0,
  sentFrames: 0,
  consoleErrors: [],
  pageErrors: [],
  requestFailures: [],
  loadingOverlayGone: false,
  bodyLength: 0,
};

const browser = await launchBrowser(chromium);
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  page.on("console", (message) => {
    if (message.type() === "error") {
      result.consoleErrors.push(message.text());
    }
  });
  page.on("pageerror", (error) => result.pageErrors.push(error.message));
  page.on("requestfailed", (request) => {
    result.requestFailures.push(`${request.url()}: ${request.failure()?.errorText ?? "request failed"}`);
  });
  page.on("websocket", (socket) => {
    result.websocketCount += 1;
    socket.on("framereceived", () => {
      result.receivedFrames += 1;
    });
    socket.on("framesent", () => {
      result.sentFrames += 1;
    });
  });

  await page.goto(url, { waitUntil: "domcontentloaded", timeout: timeoutMs });
  const deadline = Date.now() + timeoutMs;
  let bodyText = "";
  while (Date.now() < deadline) {
    bodyText = await page.locator("body").innerText();
    const loadingMarkersPresent =
      bodyText.includes("Prepare interface") &&
      bodyText.includes("Start live updates") &&
      bodyText.includes("Ready");
    result.loadingOverlayGone = !loadingMarkersPresent;
    result.bodyLength = bodyText.trim().length;
    if (
      result.loadingOverlayGone &&
      result.websocketCount > 0 &&
      result.receivedFrames > 0 &&
      result.bodyLength > 100
    ) {
      break;
    }
    await page.waitForTimeout(250);
  }

  await page.screenshot({ path: screenshotPath, fullPage: true });
  console.log(JSON.stringify(result, null, 2));

  const failures = [];
  if (!result.loadingOverlayGone) failures.push("the runtime startup overlay never completed");
  if (result.websocketCount === 0) failures.push("the mounted UI opened no runtime WebSocket");
  if (result.receivedFrames === 0) failures.push("the mounted UI received no runtime WebSocket frame");
  if (result.bodyLength <= 100) failures.push("the mounted workbench remained empty");
  if (result.consoleErrors.length > 0) failures.push("browser console errors were recorded");
  if (result.pageErrors.length > 0) failures.push("uncaught page errors were recorded");
  if (result.requestFailures.length > 0) failures.push("browser request failures were recorded");
  if (failures.length > 0) {
    throw new Error(failures.join("; "));
  }
} finally {
  await browser.close();
}
