import type { Script } from "./Script";

export class BinarySearchTree {
  #root: Node = {
    parent: null,
    depth: 0,
  };
  #script: Script;

  constructor(script: Script) {
    this.#script = script;
  }

  get children() {
    return this.#root.children;
  }

  add(input: string, output: string, type: 'default' | 'wordInitial' | 'wordFinal' | 'word' = 'default') {
    if (!input) {
      throw new Error('Empty inputs not allowed');
    }
    let node = this.#root;
    for (const char of input) {
      const map = (node.children ??= new Map<string, Node>());
      let childNode = map.get(char);
      if (!childNode) {
        childNode = {
          parent: node,
          depth: node.depth + 1,
        };
        map.set(char, childNode);
      }
      node = childNode;
    }
    node[type] = output;
  }

  findNext(text: string, index = 0) {
    let output: string | undefined;
    let node = this.#root;
    const isWordInitial = index <= 0 || !this.#script.isWordChar(text[index - 1]);
    for (let i = index; i < text.length; i++) {
      const char = text[i];
      const child = node.children?.get(char);
      if (child) {
        node = child;
      } else {
        break;
      }
    }
    while (node.parent) {
      const finalCharIndex = index + node.depth - 1;
      const isWordFinal = finalCharIndex >= text.length - 1 || !this.#script.isWordChar(text[finalCharIndex + 1]);
      if (isWordInitial && isWordFinal && node.word) {
        output = node.word;
      } else if (isWordInitial && node.wordInitial) {
        output = node.wordInitial;
      } else if (isWordFinal && node.wordFinal) {
        output = node.wordFinal;
      } else if (node.default) {
        output = node.default;
      }
      if (typeof output === 'string') {
        return {
          matchLength: node.depth,
          output,
        };
      }
      node = node.parent;
    }
  }

  // findMatch(text: string, index = 0) {
  //   let output: string | undefined;
  //   let node = this.#root;
  //   let i = index;
  //   let matchLength = 0;
  //   for (; i < text.length; i++) {
  //     const char = text[i];
  //     const newNode = node.children?.get(char);
  //     if (newNode) {
  //       if (newNode.output) {
  //         if (newNode.wi && (i <= 0 || !this.#script.isWordChar(text[i - 1]))) {
  //           output = newNode.wi;
  //         } else if (newNode.wf && (i >= text.length - 1 || !this.#script.isWordChar(text[i + 1]))) {
  //           output = newNode.wf;
  //         } else {
  //           output = newNode.output;
  //         }
  //         matchLength = i - index + 1;
  //       }
  //       node = newNode;
  //     } else {
  //       break;
  //     }
  //   }
  //   return {
  //     matchLength,
  //     output,
  //   };
  // }
}

interface Node {
  parent: Node | null;
  depth: number;
  default?: string;
  wordInitial?: string;
  wordFinal?: string;
  word?: string;
  children?: Map<string, Node>;
}
