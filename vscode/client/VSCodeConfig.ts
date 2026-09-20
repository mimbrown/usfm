import { workspace } from "vscode";
import { ConfigService } from "./ConfigService";

export class VSCodeConfig implements VSCodeConfigInterface {
  private _enable!: boolean;
  private _trace!: TraceLevel;
  private _binPath: string | undefined;
  private _requireConfig!: boolean;
  private _stylesheet: string | undefined;

  constructor() {
    this.refresh();
  }

  private get configuration() {
    return workspace.getConfiguration(ConfigService.namespace);
  }

  public refresh(): void {
    this._enable = this.configuration.get<boolean>("enable") ?? true;
    this._trace = this.configuration.get<TraceLevel>("trace.server") || "off";
    this._binPath = this.configuration.get<string>("path.server");
    this._requireConfig =
      this.configuration.get<boolean>("requireConfig") ?? false;
    this._stylesheet = this.configuration.get<string>("stylesheet") ?? undefined;
  }

  get enable(): boolean {
    return this._enable;
  }

  updateEnable(value: boolean): PromiseLike<void> {
    this._enable = value;
    return this.configuration.update("enable", value);
  }

  get trace(): TraceLevel {
    return this._trace;
  }

  updateTrace(value: TraceLevel): PromiseLike<void> {
    this._trace = value;
    return this.configuration.update("trace.server", value);
  }

  get binPath(): string | undefined {
    return this._binPath;
  }

  updateBinPath(value: string | undefined): PromiseLike<void> {
    this._binPath = value;
    return this.configuration.update("path.server", value);
  }

  get requireConfig(): boolean {
    return this._requireConfig;
  }

  get stylesheet(): string | undefined {
    return this._stylesheet;
  }

  updateStylesheet(value: string | undefined): PromiseLike<void> {
    this._stylesheet = value;
    return this.configuration.update("stylesheet", value);
  }

  updateRequireConfig(value: boolean): PromiseLike<void> {
    this._requireConfig = value;
    return this.configuration.update("requireConfig", value);
  }
}

type TraceLevel = "off" | "messages" | "verbose";

/**
 * See `"contributes.configuration"` in `package.json`
 */
interface VSCodeConfigInterface {
  /**
   * `usfm.enable`
   *
   * @default true
   */
  enable: boolean;
  /**
   * Trace VSCode <-> Usfm Language Server communication
   * `usfm.trace.server`
   *
   * @default 'off'
   */
  trace: TraceLevel;
  /**
   * Path to LSP binary
   * `usfm.path.server`
   * @default undefined
   */
  binPath: string | undefined;
  /**
   * Start the language server only when a `usfm.config.json` file exists in one of the workspaces.
   * `usfm.requireConfig`
   * @default false
   */
  requireConfig: boolean;
  /**
   * Path to the project's USFM stylesheet (`.sty`), absolute or relative to the
   * workspace folder. The server extends the default stylesheet with it; with
   * no setting it looks for a `custom.sty` beside the open file.
   * `usfm.stylesheet`
   * @default undefined
   */
  stylesheet: string | undefined;
}
