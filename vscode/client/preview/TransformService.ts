import { spawn, type ChildProcess } from "node:child_process";
import type { Preview } from "../types/configurationSchema";
import { join, parse, resolve } from "node:path";
import { Resource } from "../Resource";
import { EventEmitter, type TextEditor } from "vscode";

export class TransformService extends Resource {
  #connection: Connection | undefined;
  #args: string[];
  #extensionPath: string;

  constructor(
    parentSignal: AbortSignal,
    extensionPath: string,
    root: string,
    preview: Preview,
    input: TextEditor
  ) {
    super(parentSignal);
    this.#extensionPath = extensionPath;
    const args = [input.document.uri.fsPath] as string[];
    if (preview.replacements) {
      for (const replacement of preview.replacements) {
        args.push("-r", resolve(root, replacement));
      }
    }
    if (preview.style) {
      args.push("-s", resolve(root, preview.style));
    }
    if (preview.diglot) {
      const { base } = parse(input.document.uri.fsPath);
      args.push("-d", resolve(root, preview.diglot.path, base));
      if (preview.diglot.style) {
        args.push("-ds", resolve(root, preview.diglot.style));
      }
      if (preview.diglot.replacements) {
        for (const replacement of preview.diglot.replacements) {
          args.push("-dr", resolve(root, replacement));
        }
      }
    }
    this.#args = args;
  }

  ensureStarted() {
    if (!this.#connection) {
      this.#connection = this.#createConnection();
    }
    return this.#connection.promise;
  }

  get onTextChanged() {
    return this.#connection!.onTextChanged;
  }

  get text() {
    return this.#connection!.text;
  }

  #createConnection() {
    return new Connection(this.signal, this.#extensionPath, [
      "-w",
      ...this.#args,
      "-f",
      "html",
    ]);
  }
}

class Connection extends Resource {
  readonly promise: Promise<void>;
  #cp: ChildProcess;
  #text: string | undefined;
  #textChangedEvent = new EventEmitter<void>();
  onTextChanged = this.#textChangedEvent.event;

  constructor(
    parentSignal: AbortSignal,
    extensionPath: string,
    args: string[]
  ) {
    super(parentSignal);

    console.log(
      `${
        process.env.USFM_PARSER_PATH_DEV ||
        join(extensionPath, "target/release/usfm_parser")
      } ${args.join(" ")}`
    );

    this.#cp = spawn(
      process.env.USFM_PARSER_PATH_DEV ||
        join(extensionPath, "target/release/usfm_parser"),
      args,
      {
        signal: this.signal,
      }
    );
    this.#cp.stderr!.pipe(process.stderr);
    let promiseSettled = false;
    this.promise = new Promise<void>((resolve, reject) =>
      this.#cp
        .once("error", (error) => {
          if (!promiseSettled) {
            promiseSettled = true;
            reject(error);
          }
        })
        .stdout!.on("data", (data) => {
          this.#text = data.toString();
          console.log(this.#text);
          this.#textChangedEvent.fire();
          if (!promiseSettled) {
            promiseSettled = true;
            resolve();
          }
        })
    );
  }

  get text() {
    if (this.#text == null) {
      throw new Error("Connection accessed before ready");
    }
    return this.#text;
  }
}
