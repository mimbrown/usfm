export class Script {
  #whitespace: Set<string>;
  #punctuation: Set<string>;

  constructor(scriptDefinition: {
    whitespace: string;
    punctuation: string;
  }) {
    this.#whitespace = new Set(scriptDefinition.whitespace);
    this.#punctuation = new Set(scriptDefinition.punctuation);
  }

  isWordChar(char: string) {
    return !(this.#whitespace.has(char) || this.#punctuation.has(char));
  }
}
