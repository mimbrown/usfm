import {
  ConfigurationChangeEvent,
  ConfigurationTarget,
  workspace,
  WorkspaceFolder,
} from "vscode";
import { ConfigService } from "./ConfigService";

export const usfmConfigFileName = "usfm.config.json";

export type Trigger = "onSave" | "onType";

/**
 * See `"contributes.configuration"` in `package.json`
 */
export interface WorkspaceConfigInterface {
  /**
   * usfm config path
   *
   * `usfm.configPath`
   *
   * @default null
   */
  configPath: string | null;
  /**
   * When to run the linter and generate diagnostics
   * `usfm.lint.run`
   *
   * @default 'onType'
   */
  run: Trigger;
}

export class WorkspaceConfig {
  private _configPath: string | null = null;
  private _runTrigger: Trigger = "onType";

  constructor(private readonly workspace: WorkspaceFolder) {
    this.refresh();
  }

  private get configuration() {
    return workspace.getConfiguration(ConfigService.namespace, this.workspace);
  }

  public refresh(): void {
    this._runTrigger = this.configuration.get<Trigger>("lint.run") || "onType";
    this._configPath =
      this.configuration.get<string | null>("configPath") || usfmConfigFileName;
  }

  public effectsConfigChange(event: ConfigurationChangeEvent): boolean {
    if (
      event.affectsConfiguration(
        `${ConfigService.namespace}.configPath`,
        this.workspace
      )
    ) {
      return true;
    }
    if (
      event.affectsConfiguration(
        `${ConfigService.namespace}.lint.run`,
        this.workspace
      )
    ) {
      return true;
    }
    return false;
  }

  public get isCustomConfigPath(): boolean {
    return this.configPath !== null && this.configPath !== usfmConfigFileName;
  }

  get runTrigger(): Trigger {
    return this._runTrigger;
  }

  updateRunTrigger(value: Trigger): PromiseLike<void> {
    this._runTrigger = value;
    return this.configuration.update(
      "lint.run",
      value,
      ConfigurationTarget.WorkspaceFolder
    );
  }

  get configPath(): string | null {
    return this._configPath;
  }

  updateConfigPath(value: string | null): PromiseLike<void> {
    this._configPath = value;
    return this.configuration.update(
      "configPath",
      value,
      ConfigurationTarget.WorkspaceFolder
    );
  }

  public toLanguageServerConfig(): WorkspaceConfigInterface {
    return {
      run: this.runTrigger,
      configPath: this.configPath ?? null,
    };
  }
}
