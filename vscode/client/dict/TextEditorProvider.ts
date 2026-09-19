import * as vscode from "vscode";
import { Resource } from "../Resource";
import { getNonce } from "./util";

export class DictTextEditorProvider
  extends Resource
  implements vscode.CustomTextEditorProvider
{
  #context: vscode.ExtensionContext;

  constructor(parentSignal: AbortSignal, context: vscode.ExtensionContext) {
    super(parentSignal);
    this.#context = context;
    context.subscriptions.push(
      vscode.window.registerCustomEditorProvider("usfm.dict", this, {
        webviewOptions: {
          enableFindWidget: true,
        },
      })
    );
  }

  resolveCustomTextEditor(
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel,
    _token: vscode.CancellationToken
  ): Thenable<void> | void {
    // Setup initial content for the webview
    webviewPanel.webview.options = {
      enableScripts: true,
    };
    webviewPanel.webview.html = this.getHtmlForWebview(webviewPanel.webview);

    function updateWebview() {
      webviewPanel.webview.postMessage({
        type: "update",
        text: document.getText(),
      });
    }

    // Hook up event handlers so that we can synchronize the webview with the text document.
    //
    // The text document acts as our model, so we have to sync change in the document to our
    // editor and sync changes in the editor back to the document.
    //
    // Remember that a single text document can also be shared between multiple custom
    // editors (this happens for example when you split a custom editor)

    const changeDocumentSubscription = vscode.workspace.onDidChangeTextDocument(
      (e) => {
        if (e.document.uri.toString() === document.uri.toString()) {
          updateWebview();
        }
      }
    );

    // Make sure we get rid of the listener when our editor is closed.
    webviewPanel.onDidDispose(() => {
      changeDocumentSubscription.dispose();
    });

    // Receive message from the webview.
    webviewPanel.webview.onDidReceiveMessage((e) => {
      switch (e.type) {
        case "update": {
          this.updateTextDocument(document, e.key, e.value, e.line);
          return;
        }
        // case "add":
        //   this.addNewScratch(document);
        //   return;
        // case "delete":
        //   this.deleteScratch(document, e.id);
        //   return;
      }
    });

    updateWebview();
  }

  /**
   * Get the static html used for the editor webviews.
   */
  private getHtmlForWebview(webview: vscode.Webview): string {
    // Local path to script and css for the webview
    const scriptUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this.#context.extensionUri, "media", "dict.js")
    );

    const styleResetUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this.#context.extensionUri, "media", "reset.css")
    );

    const styleVSCodeUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this.#context.extensionUri, "media", "vscode.css")
    );

    const styleMainUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this.#context.extensionUri, "media", "dict.css")
    );

    // Use a nonce to whitelist which scripts can be run
    const nonce = getNonce();

    return /* html */ `
			<!DOCTYPE html>
			<html lang="en">
			<head>
				<meta charset="UTF-8">

				<!--
				Use a content security policy to only allow loading images from https or from our extension directory,
				and only allow scripts that have a specific nonce.
				-->
				<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src ${webview.cspSource}; style-src ${webview.cspSource}; script-src 'nonce-${nonce}';">

				<meta name="viewport" content="width=device-width, initial-scale=1.0">

				<link href="${styleResetUri}" rel="stylesheet" />
				<link href="${styleVSCodeUri}" rel="stylesheet" />
				<link href="${styleMainUri}" rel="stylesheet" />

				<title>Dictionary</title>
			</head>
			<body>
        <main>
          <dl class="mappings"></dl>
        </main>
				<script nonce="${nonce}" src="${scriptUri}"></script>
			</body>
			</html>`;
  }

  /**
   * Write out an edit to a given document.
   */
  private updateTextDocument(
    document: vscode.TextDocument,
    key: string,
    value: string,
    line: number
  ) {
    const edit = new vscode.WorkspaceEdit();

    edit.replace(
      document.uri,
      new vscode.Range(line, 0, line + 1, 0),
      `${key} > ${value}\n`
    );

    return vscode.workspace.applyEdit(edit);
  }
}
