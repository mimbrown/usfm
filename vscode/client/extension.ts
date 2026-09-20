import { promises as fsPromises } from "node:fs";

import {
  commands,
  ExtensionContext,
  StatusBarAlignment,
  StatusBarItem,
  ThemeColor,
  Uri,
  window,
  workspace,
} from "vscode";

import {
  ConfigurationParams,
  ExecuteCommandRequest,
  MessageType,
  ShowMessageNotification,
} from "vscode-languageclient";

import {
  Executable,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

import { join } from "node:path";
import { ConfigService } from "./ConfigService";
import { VSCodeConfig } from "./VSCodeConfig";
import { PreviewService } from "./preview/PreviewService";
import { DictTextEditorProvider } from "./dict/TextEditorProvider";

const languageClientName = "usfm";
const outputChannelName = "Usfm";
const commandPrefix = "usfm";

const enum UsfmCommands {
  RestartServer = `${commandPrefix}.restartServer`,
  ApplyAllFixesFile = `${commandPrefix}.applyAllFixesFile`,
  ShowOutputChannel = `${commandPrefix}.showOutputChannel`,
  ToggleEnable = `${commandPrefix}.toggleEnable`,
  Preview = `${commandPrefix}.preview`,
}

const enum LspCommands {
  FixAll = "usfm.fixAll",
}

let client: LanguageClient | undefined;

let abortController: AbortController | undefined;
let previewService: PreviewService | undefined;

let myStatusBarItem: StatusBarItem;

// Global flag to check if the user allows us to start the server.
// When `usfm.requireConfig` is `true`, make sure one `usfm.config.json` file is present.
let allowedToStartServer: boolean;

export async function activate(context: ExtensionContext) {
  abortController = new AbortController();
  previewService = new PreviewService(abortController.signal, context);
  const configService = new ConfigService();
  allowedToStartServer = configService.vsCodeConfig.requireConfig
    ? (
        await workspace.findFiles(
          `**/usfm.config.json`,
          "**/node_modules/**",
          1
        )
      ).length > 0
    : true;

  const restartCommand = commands.registerCommand(
    UsfmCommands.RestartServer,
    async () => {
      if (client === undefined) {
        window.showErrorMessage("usfm client not found");
        return;
      }

      try {
        if (client.isRunning()) {
          await client.restart();
          window.showInformationMessage("usfm server restarted.");
        } else {
          await client.start();
        }
      } catch (err) {
        client.error("Restarting client failed", err, "force");
      }
    }
  );

  const showOutputCommand = commands.registerCommand(
    UsfmCommands.ShowOutputChannel,
    () => {
      client?.outputChannel?.show();
    }
  );

  const toggleEnable = commands.registerCommand(
    UsfmCommands.ToggleEnable,
    async () => {
      await configService.vsCodeConfig.updateEnable(
        !configService.vsCodeConfig.enable
      );

      if (client === undefined || !allowedToStartServer) {
        return;
      }

      if (client.isRunning()) {
        if (!configService.vsCodeConfig.enable) {
          await client.stop();
        }
      } else {
        if (configService.vsCodeConfig.enable) {
          await client.start();
        }
      }
    }
  );

  const applyAllFixesFile = commands.registerCommand(
    UsfmCommands.ApplyAllFixesFile,
    async () => {
      if (!client) {
        window.showErrorMessage("usfm client not found");
        return;
      }
      const textEditor = window.activeTextEditor;
      if (!textEditor) {
        window.showErrorMessage("active text editor not found");
        return;
      }

      const params = {
        command: LspCommands.FixAll,
        arguments: [
          {
            uri: textEditor.document.uri.toString(),
          },
        ],
      };

      await client.sendRequest(ExecuteCommandRequest.type, params);
    }
  );

  const previewCommand = commands.registerCommand(
    UsfmCommands.Preview,
    async () => {
      if (!previewService) {
        return;
      }
      const textEditor = window.activeTextEditor;
      if (!textEditor) {
        window.showErrorMessage("Unable to find file to preview");
        return;
      }
      previewService.previewEditor(textEditor);
    }
  );

  const outputChannel = window.createOutputChannel(outputChannelName, {
    log: true,
  });

  context.subscriptions.push(
    applyAllFixesFile,
    restartCommand,
    showOutputCommand,
    toggleEnable,
    previewCommand,
    configService,
    outputChannel
  );

  new DictTextEditorProvider(abortController.signal, context);

  async function findBinary(): Promise<string> {
    let bin = configService.getUserServerBinPath();
    if (bin) {
      try {
        await fsPromises.access(bin);
        return bin;
      } catch (e) {
        outputChannel.error(`Invalid bin path: ${bin}`, e);
      }
    }
    const ext = process.platform === "win32" ? ".exe" : "";
    // NOTE: The `./target/release` path is aligned with the path defined in .github/workflows/release_vscode.yml
    // The binary is `apps/usfm_language_server`'s, built by `npm run
    // server:build:release` (ticket 30); the parked `wip/` server is gone.
    return (
      process.env.SERVER_PATH_DEV ??
      join(context.extensionPath, `./target/release/usfm-language-server${ext}`)
    );
  }

  const command = await findBinary();
  const run: Executable = {
    command: command!,
    options: {
      env: {
        ...process.env,
        RUST_LOG: process.env.RUST_LOG || "info",
      },
    },
  };
  const serverOptions: ServerOptions = {
    run,
    debug: run,
  };

  // If the extension is launched in debug mode then the debug server options are used
  // Otherwise the run options are used
  // Options to control the language client
  let clientOptions: LanguageClientOptions = {
    // Register the server for plain text documents
    documentSelector: [
      {
        language: "usfm",
        scheme: "file",
      },
    ],
    initializationOptions: serverInitializationOptions(configService),
    outputChannel,
    traceOutputChannel: outputChannel,
    middleware: {
      workspace: {
        configuration: (params: ConfigurationParams) => {
          return params.items.map((item) => {
            if (item.section !== "usfm_language_server") {
              return null;
            }
            if (item.scopeUri === undefined) {
              return null;
            }

            return (
              configService
                .getWorkspaceConfig(Uri.parse(item.scopeUri))
                ?.toLanguageServerConfig() ?? null
            );
          });
        },
      },
    },
  };

  // Create the language client and start the client.
  client = new LanguageClient(languageClientName, serverOptions, clientOptions);

  // The server sends `window/showMessage` for one thing only: a project
  // stylesheet it could not read (ticket 30). Show it, and log the rest.
  const onNotificationDispose = client.onNotification(
    ShowMessageNotification.type,
    (params) => {
      switch (params.type) {
        case MessageType.Debug:
          outputChannel.debug(params.message);
          break;
        case MessageType.Log:
          outputChannel.info(params.message);
          break;
        case MessageType.Info:
          window.showInformationMessage(params.message);
          break;
        case MessageType.Warning:
          window.showWarningMessage(params.message);
          break;
        case MessageType.Error:
          window.showErrorMessage(params.message);
          break;
        default:
          outputChannel.info(params.message);
      }
    }
  );

  context.subscriptions.push(onNotificationDispose);

  const onDeleteFilesDispose = workspace.onDidDeleteFiles((event) => {
    for (const fileUri of event.files) {
      client?.diagnostics?.delete(fileUri);
    }
  });

  context.subscriptions.push(onDeleteFilesDispose);

  const onDidChangeWorkspaceFoldersDispose =
    workspace.onDidChangeWorkspaceFolders(async (event) => {
      for (const folder of event.added) {
        configService.addWorkspaceConfig(folder);
      }
      for (const folder of event.removed) {
        configService.removeWorkspaceConfig(folder);
      }
    });

  context.subscriptions.push(onDidChangeWorkspaceFoldersDispose);

  configService.onConfigChange = async function onConfigChange(event) {
    updateStatsBar(context, this.vsCodeConfig.enable);

    if (client === undefined) {
      return;
    }

    // update the initializationOptions for a possible restart
    client.clientOptions.initializationOptions =
      serverInitializationOptions(this);

    if (
      configService.effectsWorkspaceConfigChange(event) &&
      client.isRunning()
    ) {
      await client.sendNotification("workspace/didChangeConfiguration", {
        settings: this.languageServerConfig,
      });
    }
  };

  updateStatsBar(context, configService.vsCodeConfig.enable);
  if (allowedToStartServer) {
    if (configService.vsCodeConfig.enable) {
      await client?.start();
    }
  } else {
    generateActivatorByConfig(configService.vsCodeConfig, context);
  }
}

/**
 * What the server is told at `initialize`.
 *
 * `stylesheet` is the one key `apps/usfm_language_server` reads (ticket 30): a
 * path to the project's `.sty`, which it adds to the default stylesheet rather
 * than replacing it. With no setting, the server looks for a `custom.sty`
 * beside the open file. `workspaces` is the per-folder configuration the
 * server ignores for now; it is sent so an older client and a newer server
 * agree on the shape.
 */
function serverInitializationOptions(configService: ConfigService) {
  return {
    stylesheet: configService.vsCodeConfig.stylesheet ?? null,
    workspaces: configService.languageServerConfig,
  };
}

export async function deactivate(): Promise<void> {
  if (abortController) {
    abortController.abort();
    abortController = undefined;
    previewService = undefined;
  }
  if (client) {
    await client.stop();
    client = undefined;
  }
}

function updateStatsBar(context: ExtensionContext, enable: boolean) {
  if (!myStatusBarItem) {
    myStatusBarItem = window.createStatusBarItem(StatusBarAlignment.Right, 100);
    myStatusBarItem.command = UsfmCommands.ToggleEnable;
    context.subscriptions.push(myStatusBarItem);
    myStatusBarItem.show();
  }
  let bgColor: string;
  let icon: string;
  if (!allowedToStartServer) {
    bgColor = "statusBarItem.offlineBackground";
    icon = "$(circle-slash)";
  } else if (!enable) {
    bgColor = "statusBarItem.warningBackground";
    icon = "$(check)";
  } else {
    bgColor = "statusBarItem.activeBackground";
    icon = "$(check-all)";
  }

  myStatusBarItem.text = `${icon} usfm`;
  myStatusBarItem.backgroundColor = new ThemeColor(bgColor);
}

function generateActivatorByConfig(
  config: VSCodeConfig,
  context: ExtensionContext
): void {
  const watcher = workspace.createFileSystemWatcher(
    "**/usfm.config.json",
    false,
    true,
    true
  );
  watcher.onDidCreate(async () => {
    watcher.dispose();
    allowedToStartServer = true;
    updateStatsBar(context, config.enable);
    if (client && !client.isRunning() && config.enable) {
      await client.start();
    }
  });

  context.subscriptions.push(watcher);
}
