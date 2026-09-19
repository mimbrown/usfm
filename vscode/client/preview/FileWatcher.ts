import { workspace, EventEmitter, type Uri } from "vscode";
import { Resource } from "../Resource";
import { unwatchFile, watchFile } from "node:fs";

export class FileWatcher extends Resource {
  #changeEvent = new EventEmitter<void>();
  onChange = this.#changeEvent.event;
  #onChange = () => {
    this.#changeEvent.fire();
  };

  constructor(parentSignal: AbortSignal, files: string[]) {
    super(parentSignal);
    for (const file of new Set(files)) {
      const watcher = watchFile(file, this.#onChange);
      this.onDispose(() => {
        unwatchFile(file, this.#onChange);
      });
    }
  }
  // #changeEvent = new EventEmitter<Uri>();
  // onChange = this.#changeEvent.event;
  // #onChange = (uri: Uri) => {
  //   this.#changeEvent.fire(uri);
  // };

  // constructor(parentSignal: AbortSignal, files: string[]) {
  //   super(parentSignal);
  //   for (const file of new Set(files)) {
  //     const watcher = workspace.createFileSystemWatcher(
  //       file,
  //       false,
  //       false,
  //       false
  //     );

  //     this.onDispose(watcher.onDidChange(this.#onChange));
  //     this.onDispose(watcher.onDidCreate(this.#onChange));
  //     this.onDispose(watcher.onDidDelete(this.#onChange));
  //   }
  // }
}
