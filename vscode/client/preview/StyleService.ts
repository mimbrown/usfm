import type { Preview } from "../types/configurationSchema";
import { resolve } from "node:path";
import { FileWatcher } from "./FileWatcher";
import { readFile } from "node:fs/promises";
import { Resource } from "../Resource";
import { EventEmitter } from "vscode";

export class StyleService extends Resource {
  #promise: Promise<void> | undefined;
  #files: string[];
  #fileWatcher: FileWatcher | undefined;
  #stylesheet = "";
  #changeEvent = new EventEmitter<void>();
  onChange = this.#changeEvent.event;

  constructor(parentSignal: AbortSignal, root: string, preview: Preview) {
    super(parentSignal);
    this.#files =
      preview.stylesheets?.map((stylesheet) => resolve(root, stylesheet)) ?? [];
    this.onDispose(() => {
      if (this.#timeout) {
        clearTimeout(this.#timeout);
      }
    });
  }

  get stylesheet() {
    return this.#stylesheet;
  }

  get ready() {
    if (!this.#promise) {
      this.#promise = this.#buildStylesheet();
      this.#fileWatcher = new FileWatcher(this.signal, this.#files);
      this.#fileWatcher.onChange(() => {
        this.#schedule();
      });
    }
    return this.#promise;
  }

  #timeout: NodeJS.Timeout | undefined;

  #schedule() {
    if (this.#timeout) {
      return;
    }
    this.#timeout = setTimeout(() => {
      this.#timeout = undefined;
      this.#promise = this.#buildStylesheet().then(() => {
        this.#changeEvent.fire();
      });
    }, 300);
  }

  async #buildStylesheet() {
    const styles = await Promise.all(
      this.#files.map((stylesheet) =>
        readFile(stylesheet, {
          signal: this.signal,
          encoding: "utf-8",
        }).catch(() => "")
      )
    );
    this.#stylesheet = styles.join("\n");
  }
}
