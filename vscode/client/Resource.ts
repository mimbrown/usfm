import type { IDisposable } from "./types";

export class Resource implements IDisposable {
  #abortController = new AbortController();
  #signal: AbortSignal;
  #parentSignal: AbortSignal;

  constructor(parentSignal: AbortSignal) {
    if (parentSignal.aborted) {
      throw new Error("Parent signal aborted");
    }
    this.#parentSignal = parentSignal;
    this.#signal = AbortSignal.any([
      parentSignal,
      this.#abortController.signal,
    ]);
  }

  get signal() {
    return this.#signal;
  }

  get disposed() {
    return this.#signal.aborted;
  }

  onDispose(disposable: (() => void) | IDisposable) {
    const cb =
      typeof disposable === "function"
        ? disposable
        : () => disposable.dispose();
    this.#signal.addEventListener("abort", cb);
  }

  dispose() {
    if (!this.disposed) {
      this.#abortController.abort();
    }
  }
}
