const fs = require("fs");
const path = require("path");
const { execFileSync, spawn } = require("child_process");
const vscode = require("vscode");

const output = vscode.window.createOutputChannel("Earth");

function configuredPath(name) {
  const value = vscode.workspace.getConfiguration("earth").get(name);
  return typeof value === "string" && value.trim() ? value.trim() : "";
}

function executableCandidates(name) {
  if (name === "cli") {
    const home = process.env.USERPROFILE || process.env.HOME;
    if (process.platform === "win32") {
      const localAppData = process.env.LOCALAPPDATA;
      return [
        ...(localAppData
          ? [path.join(localAppData, "Programs", "Earth", "earth.exe")]
          : []),
        ...(home ? [path.join(home, ".earth", "earth.exe")] : []),
        "earth.exe",
      ];
    }

    return [
      ...(home ? [path.join(home, ".local", "bin", "earth")] : []),
      "/usr/local/bin/earth",
      ...(process.platform === "darwin" ? ["/opt/homebrew/bin/earth"] : []),
      "earth",
    ];
  }

  if (process.platform === "win32") {
    const localAppData = process.env.LOCALAPPDATA;
    return [
      ...(localAppData
        ? [path.join(localAppData, "Programs", "Earth Studio", "earth-studio.exe")]
        : []),
      "earth-studio.exe",
    ];
  }

  return ["earth-studio"];
}

function findExecutable(kind) {
  const configured =
    kind === "cli"
      ? configuredPath("cliPath") || configuredPath("executablePath")
      : configuredPath("studioPath");
  if (configured) {
    return configured;
  }

  const candidates = executableCandidates(kind);
  for (const candidate of candidates) {
    if (path.isAbsolute(candidate) && fs.existsSync(candidate)) {
      return candidate;
    }
  }

  const command = kind === "cli" ? "earth" : "earth-studio";
  try {
    const resolver = process.platform === "win32" ? "where.exe" : "which";
    const resolved = execFileSync(resolver, [command], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    })
      .split(/\r?\n/)
      .map((line) => line.trim())
      .find(Boolean);
    if (resolved) {
      return resolved;
    }
  } catch {
    // The caller reports a clear installation/configuration error.
  }
  return undefined;
}

async function currentEarthFile() {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== "earth") {
    await vscode.window.showErrorMessage(
      "Önce bir .earth dosyası açın.",
      "Earth dosyası aç"
    ).then((action) => {
      if (action === "Earth dosyası aç") {
        vscode.commands.executeCommand("workbench.action.files.openFile");
      }
    });
    return undefined;
  }

  if (editor.document.isDirty && !(await editor.document.save())) {
    await vscode.window.showErrorMessage(
      "Earth dosyası kaydedilemedi; çalıştırma iptal edildi."
    );
    return undefined;
  }
  return editor.document.uri.fsPath;
}

function runEarthFile(file) {
  const executable = findExecutable("cli");
  if (!executable) {
    vscode.window.showErrorMessage(
      "Earth CLI bulunamadı. Earth CLI'yi PATH'e ekleyin veya earth.cliPath ayarını belirtin.",
      "Ayarları aç",
      "CLI edinme sayfası"
    ).then((action) => {
      if (action === "Ayarları aç") {
        vscode.commands.executeCommand(
          "workbench.action.openSettings",
          "earth.cliPath"
        );
      } else if (action === "CLI edinme sayfası") {
        vscode.env.openExternal(
          vscode.Uri.parse(
            "https://github.com/MahmutSibal/earth-lang/releases/latest"
          )
        );
      }
    });
    return;
  }

  output.clear();
  output.show(true);
  output.appendLine(`$ ${executable} "${file}"`);

  const child = spawn(executable, [file], {
    cwd: path.dirname(file),
    windowsHide: true,
  });
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (data) => output.append(data));
  child.stderr.on("data", (data) => output.append(data));
  child.on("error", (error) => {
    output.appendLine(`\nEarth çalıştırılamadı: ${error.message}`);
    vscode.window.showErrorMessage(
      `Earth çalıştırılamadı: ${error.message}`
    );
  });
  child.on("close", (code, signal) => {
    if (signal) {
      output.appendLine(`\nEarth durduruldu (${signal}).`);
    } else {
      output.appendLine(`\nEarth çıkış kodu: ${code ?? "bilinmiyor"}`);
    }
  });
}

function openInStudio(file) {
  const executable = findExecutable("studio");
  if (!executable) {
    vscode.window.showErrorMessage(
      "Earth Studio bulunamadı. Earth Studio'yu kurun veya earth.studioPath ayarını belirtin.",
      "Ayarları aç",
      "İndirme sayfası"
    ).then((action) => {
      if (action === "Ayarları aç") {
        vscode.commands.executeCommand(
          "workbench.action.openSettings",
          "earth.studioPath"
        );
      } else if (action === "İndirme sayfası") {
        vscode.env.openExternal(
          vscode.Uri.parse(
            "https://github.com/MahmutSibal/earth-lang/releases/latest"
          )
        );
      }
    });
    return;
  }

  const child = spawn(executable, [file], {
    cwd: path.dirname(file),
    detached: true,
    stdio: "ignore",
    windowsHide: true,
  });
  child.unref();
  child.on("error", (error) => {
    vscode.window.showErrorMessage(
      `Earth Studio açılamadı: ${error.message}`
    );
  });
}

async function runCurrentFile() {
  const file = await currentEarthFile();
  if (file) {
    runEarthFile(file);
  }
}

async function openCurrentFileInStudio() {
  const file = await currentEarthFile();
  if (file) {
    openInStudio(file);
  }
}

function activate(context) {
  context.subscriptions.push(
    output,
    vscode.commands.registerCommand("earth.runFile", runCurrentFile),
    vscode.commands.registerCommand(
      "earth.openInStudio",
      openCurrentFileInStudio
    ),
    vscode.commands.registerCommand("earth.openSettings", () =>
      vscode.commands.executeCommand(
        "workbench.action.openSettings",
        "@ext:mahmutsibal.earth-lang"
      )
    )
  );
}

function deactivate() {}

module.exports = { activate, deactivate };
