import {
  EventEmitter,
  ViewColumn,
  window,
  workspace,
  type ExtensionContext,
  type TextEditor,
  type WorkspaceFolder,
} from "vscode";
import { join, parse, resolve } from "node:path";
import type { Configuration, Preview } from "../types/configurationSchema.ts";
import { StyleService } from "./StyleService";
import { TransformService } from "./TransformService";
import { readFile } from "node:fs/promises";
import { Resource } from "../Resource";
import { minimatch } from "minimatch";

export class PreviewService extends Resource {
  #workers: Map<string, WorkspacePreviewWorker> = new Map();
  #context: ExtensionContext;

  constructor(parentSignal: AbortSignal, context: ExtensionContext) {
    super(parentSignal);
    this.#context = context;
  }

  async previewEditor(textEditor: TextEditor) {
    const workspaceFolder = workspace.getWorkspaceFolder(
      textEditor.document.uri
    );
    const worker = await this.#getWorker(workspaceFolder);
    worker.preview(textEditor);
  }

  async #getWorker(workspaceFolder: WorkspaceFolder | undefined) {
    const key = workspaceFolder?.uri.path ?? "";
    let worker = this.#workers.get(key);
    if (worker) {
      return worker;
    }
    worker = new WorkspacePreviewWorker(
      this.signal,
      this.#context,
      workspaceFolder,
      await this.#getConfiguration(workspaceFolder)
    );
    this.#workers.set(key, worker);
    return worker;
  }

  async #getConfiguration(
    workspaceFolder: WorkspaceFolder | undefined
  ): Promise<Configuration> {
    try {
      const text = await readFile(
        join(workspaceFolder?.uri.fsPath ?? "", "usfm.config.json"),
        {
          signal: this.signal,
          encoding: "utf-8",
        }
      );
      return JSON.parse(text);
    } catch {
      return {};
    }
  }
}

interface PreviewOption {
  preview: Preview;
  includes: string[];
  styleService: StyleService;
}

export class WorkspacePreviewWorker extends Resource {
  #previewOptions: PreviewOption[];
  #activePreviews: FilePreview[] = [];
  #context: ExtensionContext;
  #workspaceFolder: WorkspaceFolder | undefined;

  constructor(
    parentSignal: AbortSignal,
    context: ExtensionContext,
    workspaceFolder: WorkspaceFolder | undefined,
    configuration: Configuration
  ) {
    super(parentSignal);
    this.#context = context;
    this.#workspaceFolder = workspaceFolder;
    this.#previewOptions = (
      configuration.previews?.length
        ? configuration.previews
        : [{ title: "Default" }]
    ).map((preview) => ({
      preview,
      includes:
        preview.includes?.map((pattern) =>
          resolve(workspaceFolder?.uri.fsPath ?? "", pattern)
        ) ?? [],
      styleService: new StyleService(
        this.signal,
        workspaceFolder?.uri.fsPath ?? "",
        preview
      ),
    }));
  }

  preview(textEditor: TextEditor) {
    const filePreview = new FilePreview(
      this.signal,
      this.#context,
      this.#workspaceFolder,
      textEditor,
      this.#previewOptions.filter((option) => {
        if (option.includes.length) {
          return option.includes.some((pattern) => {
            return minimatch(textEditor.document.fileName, pattern);
          });
        }
        return true;
      })
    );
    filePreview.onDispose(() => {
      if (this.disposed) {
        return;
      }
      this.#activePreviews = this.#activePreviews.filter(
        (preview) => preview !== filePreview
      );
    });
    this.#activePreviews.push(filePreview);
  }
}

class FilePreview extends Resource {
  #connection: WebviewContent;
  #context: ExtensionContext;
  #workspaceFolder: WorkspaceFolder | undefined;

  constructor(
    parentSignal: AbortSignal,
    context: ExtensionContext,
    workspaceFolder: WorkspaceFolder | undefined,
    textEditor: TextEditor,
    previewOptions: PreviewOption[]
  ) {
    super(parentSignal);
    this.#context = context;
    this.#workspaceFolder = workspaceFolder;
    const { base } = parse(textEditor.document.fileName);
    const panel = window.createWebviewPanel(
      "usfmPreview",
      `${base} [${
        previewOptions.length > 1 ? previewOptions[0].preview.title : "Preview"
      }]`,
      {
        viewColumn: textEditor.viewColumn ?? ViewColumn.One,
        preserveFocus: true,
      },
      {
        enableScripts: true,
        enableFindWidget: true,
      }
    );

    let optionIndex = 0;

    this.#connection = new WebviewContent(
      this.signal,
      this.#context,
      this.#workspaceFolder,
      textEditor,
      previewOptions[optionIndex]
    );

    const updateHtml = () => {
      panel.webview.html = `<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${base} [${
        previewOptions.length > 1 ? previewOptions[0].preview.title : "Preview"
      }]</title>
  <style id="stylesheet">${this.#connection.stylesheet}</style>
  <script type="module">
    const vscode = acquireVsCodeApi();
    document.querySelector("#preview-select")?.addEventListener("change", (event) => {
      vscode.postMessage({ type: "preview", index: Number(event.target.value) });
    });
  </script>
</head>
<body>
  <header style="display: flex; height: 56px; align-items: center; justify-content: space-between; padding: 0 1rem;">
    <button onclick="window.print()">Print</button>
  
  ${
    previewOptions.length > 1
      ? `<select id="preview-select">${previewOptions.map(
          (option, index) =>
            `<option value="${index}"${
              index === optionIndex ? " selected" : ""
            }>${option.preview.title}</option>`
        )}</select>`
      : ""
  }
  </header>
  <main${
    previewOptions[optionIndex].preview.diglot ? ' class="diglot"' : ""
  } style="padding-bottom: 500px">${this.#connection.content}</main>
</body>
</html>`;
    };

    this.onDispose(this.#connection.onContentUpdate(updateHtml));

    this.onDispose(
      panel.webview.onDidReceiveMessage((message) => {
        if (message.type === "preview") {
          const option = previewOptions[message.index];
          optionIndex = message.index;
          panel.title = `${base} [${option.preview.title}]`;
          this.#connection.dispose();
          this.#connection = new WebviewContent(
            this.signal,
            this.#context,
            this.#workspaceFolder,
            textEditor,
            option
          );
          this.onDispose(this.#connection.onContentUpdate(updateHtml));
          updateHtml();
        }
      })
    );

    this.onDispose(
      panel.onDidDispose(() => {
        this.dispose();
      })
    );

    this.onDispose(() => {
      panel.dispose();
    });
  }
}

class WebviewContent extends Resource {
  #option: PreviewOption;
  #content = "";
  #contentUpdateEvent = new EventEmitter<void>();

  onContentUpdate = this.#contentUpdateEvent.event;

  constructor(
    parentSignal: AbortSignal,
    context: ExtensionContext,
    workspaceFolder: WorkspaceFolder | undefined,
    textEditor: TextEditor,
    option: PreviewOption
  ) {
    super(parentSignal);
    this.#option = option;

    const { styleService } = this.#option;
    const transformService = new TransformService(
      this.signal,
      context.extensionPath,
      workspaceFolder?.uri.fsPath ?? "",
      option.preview,
      textEditor
    );

    Promise.all([styleService.ready, transformService.ensureStarted()]).then(
      () => {
        this.content = transformService.text;
      }
    );

    this.onDispose(styleService.onChange(() => this.#schedule()));
    this.onDispose(
      transformService.onTextChanged(() => {
        this.content = transformService.text;
      })
    );
  }

  #scheduled = false;
  #schedule() {
    if (this.#scheduled) {
      return;
    }
    this.#scheduled = true;
    queueMicrotask(() => {
      this.#scheduled = false;
      this.#contentUpdateEvent.fire();
    });
  }

  get stylesheet() {
    return this.#option.styleService.stylesheet;
  }

  get content() {
    return this.#content;
  }

  set content(text: string) {
    this.#content = text;
    this.#schedule();
  }
}
