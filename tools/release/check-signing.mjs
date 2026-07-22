const required = process.env.GC_REQUIRE_SIGNING === "1";

if (!required) {
  console.log(
    "[release] signing preflight is optional for this local package build",
  );
  process.exit(0);
}

const requiredVariables =
  process.platform === "win32"
    ? ["WINDOWS_CERTIFICATE_THUMBPRINT", "WINDOWS_TIMESTAMP_URL"]
    : process.platform === "darwin"
      ? ["APPLE_CERTIFICATE", "APPLE_CERTIFICATE_PASSWORD"]
      : ["SIGN_KEY"];
const missing = requiredVariables.filter((name) => !process.env[name]?.trim());

if (process.platform === "darwin") {
  const hasAppleId = ["APPLE_ID", "APPLE_PASSWORD", "APPLE_TEAM_ID"].every(
    (name) => process.env[name]?.trim(),
  );
  const hasApiKey = ["APPLE_API_KEY", "APPLE_API_ISSUER"].every((name) =>
    process.env[name]?.trim(),
  );
  if (!hasAppleId && !hasApiKey) {
    missing.push("APPLE_ID credentials or APPLE_API_KEY credentials");
  }
}

if (missing.length > 0) {
  throw new Error(
    `signed package build is missing release credentials: ${missing.join(", ")}`,
  );
}

console.log(`[release] signing preflight passed for ${process.platform}`);
