const { spawn } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");
const {macPackagingArgs}=require('./mac-helper.cjs');

function removeAppleDouble(directory) {
  if (!fs.existsSync(directory)) return;
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const full = path.join(directory, entry.name);
    if (entry.name.startsWith("._") && entry.isFile()) fs.unlinkSync(full);
    else if (entry.isDirectory() && !["node_modules", "target", "dist"].includes(entry.name)) removeAppleDouble(full);
  }
}

removeAppleDouble(path.join(__dirname, ".."));

// An internal build directory avoids macOS AppleDouble metadata in generated
// Rust permission files when the project is stored on an external volume.
const env = { ...process.env, CARGO_TARGET_DIR: path.resolve(process.env.CARGO_TARGET_DIR || path.join(os.homedir(), ".cache", "deskbot", "target")) };
const cli = path.join(__dirname, "..", "node_modules", "@tauri-apps", "cli", "tauri.js");
const args=macPackagingArgs(process.argv.slice(2),env.CARGO_TARGET_DIR,env);
const child = spawn(process.execPath, [cli, ...args], { stdio: "inherit", env });
child.on("exit", (code, signal) => {
  if (signal) process.kill(process.pid, signal);
  else process.exit(code ?? 1);
});
