import readline from "node:readline/promises";
import { stdin, stdout } from "node:process";

export async function ask(question: string): Promise<string> {
  const rl = readline.createInterface({ input: stdin, output: stdout });
  try {
    const ans = await rl.question(question);
    return ans.trim();
  } finally {
    rl.close();
  }
}

export async function askYesNo(
  question: string,
  defaultYes = false
): Promise<boolean> {
  const suffix = defaultYes ? " [Y/n] " : " [y/N] ";
  const ans = (await ask(question + suffix)).toLowerCase();
  if (!ans) return defaultYes;
  return ans === "y" || ans === "yes";
}

export async function askMultiline(
  prompt: string,
  sentinel = "."
): Promise<string> {
  // eslint-disable-next-line no-console
  console.log(`${prompt}\n(end with a single '${sentinel}' on its own line)`);
  const rl = readline.createInterface({ input: stdin, output: stdout });
  const lines: string[] = [];
  for await (const line of rl) {
    if (line.trim() === sentinel) break;
    lines.push(line);
  }
  rl.close();
  return lines.join("\n").trim();
}

export async function askChoice<T extends string>(
  prompt: string,
  choices: Array<{ key: T; label: string }>
): Promise<T> {
  // eslint-disable-next-line no-console
  console.log(`\n${prompt}`);
  choices.forEach((c, i) => {
    // eslint-disable-next-line no-console
    console.log(`  ${i + 1}) ${c.label}`);
  });
  while (true) {
    const ans = await ask("Choose [number]: ");
    const idx = parseInt(ans, 10);
    if (!isNaN(idx) && idx >= 1 && idx <= choices.length) {
      return choices[idx - 1]!.key;
    }
    // eslint-disable-next-line no-console
    console.log("Invalid choice.");
  }
}
