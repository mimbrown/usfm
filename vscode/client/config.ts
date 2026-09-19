import {
  ConfigurationChangeEvent,
  workspace,
  WorkspaceConfiguration,
} from "vscode";
import { IDisposable } from "./types";

export class ConfigService implements Config, IDisposable {
  static readonly #namespace = "usfm";
  private readonly _disposables: IDisposable[] = [];
  private _inner: WorkspaceConfiguration;
  // private _runTrigger: Trigger;
  private _enable: boolean;
  // private _trace: TraceLevel;
  private _configPath: string;
  // private _binPath: string | undefined;

  public onConfigChange:
    | ((this: ConfigService, config: ConfigurationChangeEvent) => void)
    | undefined;

  constructor() {
    this._inner = workspace.getConfiguration(ConfigService.#namespace);
    // this._runTrigger = this._inner.get<Trigger>('lint.run') || 'onType';
    this._enable = this._inner.get<boolean>("enable") ?? true;
    // this._trace = this._inner.get<TraceLevel>('trace.server') || 'off';
    this._configPath =
      this._inner.get<string>("configPath") || "usfm.config.json";
    // this._binPath = this._inner.get<string>('path.server');
    this.onConfigChange = undefined;

    const disposeChangeListener = workspace.onDidChangeConfiguration(
      this.onVscodeConfigChange.bind(this)
    );
    this._disposables.push(disposeChangeListener);
  }

  // get runTrigger(): Trigger {
  //   return this._runTrigger;
  // }

  // set runTrigger(value: Trigger) {
  //   this._runTrigger = value;
  //   workspace
  //     .getConfiguration(ConfigService.#namespace)
  //     .update('lint.run', value);
  // }

  get enable(): boolean {
    return this._enable;
  }

  set enable(value: boolean) {
    this._enable = value;
    workspace
      .getConfiguration(ConfigService.#namespace)
      .update("enable", value);
  }

  // get trace(): TraceLevel {
  //   return this._trace;
  // }

  // set trace(value: TraceLevel) {
  //   this._trace = value;
  //   workspace
  //     .getConfiguration(ConfigService.#namespace)
  //     .update('trace.server', value);
  // }

  get configPath(): string {
    return this._configPath;
  }

  set configPath(value: string) {
    this._configPath = value;
    workspace
      .getConfiguration(ConfigService.#namespace)
      .update("configPath", value);
  }

  // get binPath(): string | undefined {
  //   return this._binPath;
  // }

  // set binPath(value: string | undefined) {
  //   this._binPath = value;
  //   workspace
  //     .getConfiguration(ConfigService.#namespace)
  //     .update('path.server', value);
  // }

  private onVscodeConfigChange(event: ConfigurationChangeEvent): void {
    if (event.affectsConfiguration(ConfigService.#namespace)) {
      // this._runTrigger = this._inner.get<Trigger>('lint.run') || 'onType';
      this._enable = this._inner.get<boolean>("enable") ?? true;
      // this._trace = this._inner.get<TraceLevel>('trace.server') || 'off';
      this._configPath =
        this._inner.get<string>("configPath") || "usfm.config.json";
      // this._binPath = this._inner.get<string>('path.server');
      this.onConfigChange?.call(this, event);
    }
  }

  dispose() {
    for (const disposable of this._disposables) {
      disposable.dispose();
    }
  }

  public toLanguageServerConfig(): LanguageServerConfig {
    return {
      // run: this.runTrigger,
      enable: this.enable,
      configPath: this.configPath,
    };
  }
}

// type Trigger = 'onSave' | 'onType';
// type TraceLevel = 'off' | 'messages' | 'verbose';

interface LanguageServerConfig {
  configPath: string;
  enable: boolean;
  // run: Trigger;
}

/**
 * See `"contributes.configuration"` in `package.json`
 */
interface Config {
  /**
   * When to run the linter and generate diagnostics
   * `usfm.lint.run`
   *
   * @default 'onType'
   */
  // runTrigger: Trigger;
  /**
   * `usfm.enable`
   *
   * @default true
   */
  enable: boolean;
  /**
   * Trace VSCode <-> Oxc Language Server communication
   * `oxc.trace.server`
   *
   * @default 'off'
   */
  // trace: TraceLevel;
  /**
   * oxlint config path
   *
   * `usfm.configPath`
   *
   * @default "usfm.config.json"
   */
  configPath: string;
  /**
   * Path to LSP binary
   * `oxc.path.server`
   * @default undefined
   */
  // binPath: string | undefined;
}
