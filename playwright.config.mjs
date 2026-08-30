export default {
  testDir: "./examples/browser",
  snapshotPathTemplate: "{testDir}/smoke.spec.mjs-snapshots/{arg}-{projectName}{ext}",
  timeout: 30_000,
  use: {
    baseURL: "http://127.0.0.1:8000",
    screenshot: "only-on-failure",
  },
  projects: [
    { name: "chromium", use: { browserName: "chromium" } },
    { name: "chromium-hidpi", use: { browserName: "chromium", deviceScaleFactor: 2 } },
    { name: "firefox", use: { browserName: "firefox" } },
    { name: "webkit", use: { browserName: "webkit" } },
  ],
};
