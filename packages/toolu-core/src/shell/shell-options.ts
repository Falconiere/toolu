/**
 * getopt-style argument splitting over resolved words, shared by the wrapper,
 * git and write-target readers. A short cluster (`-am`) is read letter by
 * letter: a value letter takes the rest of the word or the next word, and a
 * "rest" letter (`sed -i.bak`) takes whatever follows it in the same word. Long
 * options take `--name=value`, or the next word when listed as value-taking.
 */

export interface OptionSpec {
  /** Short letters that take a value: attached (`-oL`) or the next word. */
  readonly valueShort?: string;
  /** Short letters whose value is only the rest of the cluster, possibly empty (`-i`, `-i.bak`). */
  readonly restShort?: string;
  /** Long names (without `--`) that take the next word when written without `=`. */
  readonly valueLong?: readonly string[];
  /** `-<digits>` is one option (`nice -10`). */
  readonly numeric?: boolean;
  /** Stop at the first operand: the rest is a command (wrappers, git globals). */
  readonly stopAtOperand?: boolean;
}

export interface ParsedOption {
  /** A short letter (`m`) or a long name (`message`). */
  readonly name: string;
  /** The value, `null` when dynamic, `undefined` when the option takes none or it is missing. */
  readonly value: string | null | undefined;
}

export interface ParsedArgs {
  readonly options: readonly ParsedOption[];
  readonly operands: readonly (string | null)[];
  /** Index just past what was consumed; with `stopAtOperand`, where the command starts. */
  readonly next: number;
  /** A value-taking option was the last word (`git -C`): the command line is malformed. */
  readonly missingValue: boolean;
}

type Words = readonly (string | null)[];

interface Step {
  readonly options: readonly ParsedOption[];
  readonly consumed: number;
  readonly missingValue: boolean;
}

function shortCluster(word: string, following: string | null | undefined, spec: OptionSpec): Step {
  if (spec.numeric === true && /^-\d+$/.test(word)) {
    return {
      options: [{ name: word.slice(1), value: undefined }],
      consumed: 1,
      missingValue: false,
    };
  }
  const options: ParsedOption[] = [];
  for (let i = 1; i < word.length; i++) {
    const letter = word.charAt(i);
    const rest = word.slice(i + 1);
    if (spec.restShort?.includes(letter) === true) {
      options.push({ name: letter, value: rest });
      return { options, consumed: 1, missingValue: false };
    }
    if (spec.valueShort?.includes(letter) === true) {
      if (rest !== "") {
        options.push({ name: letter, value: rest });
        return { options, consumed: 1, missingValue: false };
      }
      options.push({ name: letter, value: following });
      return { options, consumed: 2, missingValue: following === undefined };
    }
    options.push({ name: letter, value: undefined });
  }
  return { options, consumed: 1, missingValue: false };
}

function longOption(word: string, following: string | null | undefined, spec: OptionSpec): Step {
  const body = word.slice(2);
  const eq = body.indexOf("=");
  if (eq !== -1) {
    return {
      options: [{ name: body.slice(0, eq), value: body.slice(eq + 1) }],
      consumed: 1,
      missingValue: false,
    };
  }
  if (spec.valueLong?.includes(body) === true) {
    return {
      options: [{ name: body, value: following }],
      consumed: 2,
      missingValue: following === undefined,
    };
  }
  return { options: [{ name: body, value: undefined }], consumed: 1, missingValue: false };
}

/** Split `words[start..]` into options and operands under `spec`. */
export function parseArgs(words: Words, start: number, spec: OptionSpec): ParsedArgs {
  const options: ParsedOption[] = [];
  const operands: (string | null)[] = [];
  let i = start;
  let missingValue = false;
  while (i < words.length) {
    const word = words[i] ?? null;
    if (word === "--") {
      const rest = words.slice(i + 1);
      if (spec.stopAtOperand === true) return { options, operands, next: i + 1, missingValue };
      return { options, operands: [...operands, ...rest], next: words.length, missingValue };
    }
    if (word === null || word === "-" || !word.startsWith("-")) {
      if (spec.stopAtOperand === true) return { options, operands, next: i, missingValue };
      operands.push(word);
      i += 1;
      continue;
    }
    const step = word.startsWith("--")
      ? longOption(word, words[i + 1], spec)
      : shortCluster(word, words[i + 1], spec);
    options.push(...step.options);
    missingValue ||= step.missingValue;
    i += step.consumed;
  }
  return { options, operands, next: Math.min(i, words.length), missingValue };
}

/** Values of every option named in `names`, in order. */
export function optionValues(
  parsed: ParsedArgs,
  names: readonly string[],
): readonly (string | null | undefined)[] {
  return parsed.options.filter((option) => names.includes(option.name)).map((o) => o.value);
}

/** Whether any option named in `names` was given. */
export function hasOption(parsed: ParsedArgs, names: readonly string[]): boolean {
  return parsed.options.some((option) => names.includes(option.name));
}
