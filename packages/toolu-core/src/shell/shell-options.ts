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
  /** Space-separated long names (without `--`) that take the next word when written without `=`. */
  readonly valueLong?: string;
  /** `-<digits>` is one option (`nice -10`). */
  readonly numeric?: boolean;
  /** Stop at the first operand: the rest is a command (wrappers, git globals). */
  readonly stopAtOperand?: boolean;
  /** `+x` is an option cluster too, as a shell reads it (`bash +o posix`, `bash +c`). */
  readonly plus?: boolean;
}

export interface ParsedOption {
  /** A short letter (`m`) or a long name (`message`). */
  readonly name: string;
  /** The value, `null` when dynamic, `undefined` when the option takes none or it is missing. */
  readonly value: string | null | undefined;
  /** Index of the word the value was read from, when it was a separate word. */
  readonly at: number | null;
}

export interface ParsedArgs {
  readonly options: readonly ParsedOption[];
  readonly operands: readonly (string | null)[];
  /** Index of each operand in the parsed words. */
  readonly operandAt: readonly number[];
  /** Index just past what was consumed; with `stopAtOperand`, where the command starts. */
  readonly next: number;
  /** A value-taking option was the last word (`git -C`): the command line is malformed. */
  readonly missingValue: boolean;
}

type Words = readonly (string | null)[];

/** Split `words[start..]` into options and operands under `spec`. */
export function parseArgs(words: Words, start: number, spec: OptionSpec): ParsedArgs {
  const options: ParsedOption[] = [];
  const operandAt: number[] = [];
  let missingValue = false;
  let i = start;
  const option = spec.plus === true ? /^[-+]./ : /^-/;
  /** The option's value is the next word. */
  const takeNext = (name: string): void => {
    i += 1;
    missingValue ||= i >= words.length;
    options.push({ name, value: words[i], at: i });
  };
  const done = (next: number): ParsedArgs => ({
    options,
    operands: operandAt.map((at) => words[at] ?? null),
    operandAt,
    next,
    missingValue,
  });
  for (; i < words.length; i++) {
    const word = words[i] ?? null;
    if (word === null || word === "-" || word === "--" || !option.test(word)) {
      if (spec.stopAtOperand === true) return done(word === "--" ? i + 1 : i);
      if (word === "--") {
        for (let rest = i + 1; rest < words.length; rest++) operandAt.push(rest);
        return done(words.length);
      }
      operandAt.push(i);
    } else if (word.startsWith("--")) {
      const [name = "", value] = word.slice(2).split(/=(.*)/s);
      if (value !== undefined) options.push({ name, value, at: null });
      else if (named(spec.valueLong ?? "", name)) takeNext(name);
      else options.push({ name, value: undefined, at: null });
    } else if (spec.numeric === true && /^-\d+$/.test(word)) {
      options.push({ name: word.slice(1), value: undefined, at: null });
    } else {
      for (let j = 1; j < word.length; j++) {
        const name = word.charAt(j);
        const rest = word.slice(j + 1);
        const valued = spec.valueShort?.includes(name) === true;
        if (spec.restShort?.includes(name) === true || (valued && rest !== "")) {
          options.push({ name, value: rest, at: null });
          break;
        }
        if (valued) {
          takeNext(name);
          break;
        }
        options.push({ name, value: undefined, at: null });
      }
    }
  }
  return done(words.length);
}

/** Whether `name` is one of the space-separated `names`. */
export function named(names: string, name: string): boolean {
  return ` ${names} `.includes(` ${name} `);
}

/** Values of every option among the space-separated `names`, in order. */
export function optionValues(
  parsed: ParsedArgs,
  names: string,
): readonly (string | null | undefined)[] {
  return parsed.options.filter((option) => named(names, option.name)).map((o) => o.value);
}

/** Whether any option among the space-separated `names` was given. */
export function hasOption(parsed: ParsedArgs, names: string): boolean {
  return parsed.options.some((option) => named(names, option.name));
}
