import { BinarySearchTree } from "./BinarySearchTree";
import type { Script } from "./Script";

interface ScriptMap {
  default: Record<string, string>;
  wordInitial?: Record<string, string>;
  wordFinal?: Record<string, string>;
  word?: Record<string, string>;
}

export class ScriptConverter {
  #tree: BinarySearchTree;

  constructor(script: Script, { default: defaultMap, wordInitial = {}, wordFinal = {}, word = {}}: ScriptMap) {
    const tree = new BinarySearchTree(script);
    for (const [k, v] of Object.entries(defaultMap)) {
      if (k) {
        tree.add(k, v);
      }
    }
    for (const [k, v] of Object.entries(wordInitial)) {
      if (k) {
        tree.add(k, v, 'wordInitial');
      }
    }
    for (const [k, v] of Object.entries(wordFinal)) {
      if (k) {
        tree.add(k, v, 'wordFinal');
      }
    }
    for (const [k, v] of Object.entries(word)) {
      if (k) {
        tree.add(k, v, 'word');
      }
    }
    this.#tree = tree;
  }

  convert(input: string) {
    const tree = this.#tree;
    // const wordMap = this.buildWordMap(input);
    let fullOutput = '';
    let index = 0;
    while (index < input.length) {
      // if (index in wordMap) {
      //   const { output, length } = wordMap[index];
      //   fullOutput += output;
      //   index += length;
      //   continue;
      // }
      const next = tree.findNext(input, index);
      if (next) {
        const { matchLength, output } = next;
        fullOutput += output;
        index += matchLength;
      } else {
        // console.warn(`Unable to find mapping for ${input[index]}!`);
        fullOutput += input[index];
        index += 1;
      }
    }
    return fullOutput;
  }

  // buildWordMap(input: string) {
  //   const wordMap: Record<number, {
  //     length: number;
  //     output: string;
  //   }> = {};
  //   let index = 0;
  //   while (index < input.length) {
  //     let i = index;
  //     while (i < input.length && input[i] !== ' ') {
  //       ++i;
  //     }
  //     if (i > index) {
  //       const word = input.slice(index, i);
  //       const output = this.#words[word];
  //       if (output) {
  //         wordMap[index] = {
  //           length: word.length,
  //           output,
  //         };
  //       }
  //     }
  //     index = i + 1;
  //   }
  //   return wordMap;
  // }
}
