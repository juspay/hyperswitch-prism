import type { BrowserSession } from "../drivers/browserDriver";
import type { Rule } from "../types/dsl";

export interface InterpreterContext {
  requestUrl: string;
  data: Record<string, unknown>;
}

export async function executeRule(
  session: BrowserSession,
  rule: Rule,
  ctx: InterpreterContext
): Promise<void> {
  switch (rule.action) {
    case "goto": {
      await session.goto(rule.url ?? ctx.requestUrl, {
        timeoutMs: rule.timeoutMs,
        waitUntil: rule.waitUntil
      });
      return;
    }

    case "click":
      await session.click(rule.selector, { timeoutMs: rule.timeoutMs });
      return;

    case "fill":
      await session.fill(rule.selector, rule.value, { timeoutMs: rule.timeoutMs });
      return;

    case "press":
      await session.press(rule.selector, rule.key, { timeoutMs: rule.timeoutMs });
      return;

    case "waitFor": {
      if (!rule.selector && !rule.urlContains) {
        throw new Error("waitFor requires at least one of: selector or urlContains");
      }

      // Run both conditions in parallel if both are specified
      const promises: Promise<void>[] = [];

      if (rule.selector) {
        promises.push(
          session.waitForSelector(rule.selector, {
            timeoutMs: rule.timeoutMs,
            state: rule.state
          })
        );
      }

      if (rule.urlContains) {
        promises.push(
          session.waitForUrlContains(rule.urlContains, {
            timeoutMs: rule.timeoutMs
          })
        );
      }

      await Promise.all(promises);
      return;
    }

    case "assertVisible": {
      const visible = await session.isVisible(rule.selector, {
        timeoutMs: rule.timeoutMs
      });

      if (!visible) {
        throw new Error(`Element is not visible: ${rule.selector}`);
      }
      return;
    }

    case "assertText": {
      const actual = await session.getText(rule.selector, {
        timeoutMs: rule.timeoutMs
      });

      const expected = rule.text;
      const observed = rule.caseSensitive === false ? actual.toLowerCase() : actual;
      const target = rule.caseSensitive === false ? expected.toLowerCase() : expected;
      const mode = rule.match ?? "contains";

      const matched = mode === "equals" ? observed === target : observed.includes(target);
      if (!matched) {
        throw new Error(
          `Text assertion failed for ${rule.selector}: expected (${mode}) \"${expected}\", got \"${actual}\"`
        );
      }
      return;
    }

    case "extract": {
      const raw = rule.attribute
        ? await session.getAttribute(rule.selector, rule.attribute, {
            timeoutMs: rule.timeoutMs
          })
        : await session.getText(rule.selector, {
            timeoutMs: rule.timeoutMs
          });

      if (raw === null) {
        throw new Error(`Attribute \"${rule.attribute}\" not found on ${rule.selector}`);
      }

      ctx.data[rule.as] = rule.trim === false ? raw : raw.trim();
      return;
    }

    case "extractAll": {
      const rawValues = rule.attribute
        ? await session.getAllAttribute(rule.selector, rule.attribute, {
            timeoutMs: rule.timeoutMs
          })
        : await session.getAllText(rule.selector, {
            timeoutMs: rule.timeoutMs
          });

      ctx.data[rule.as] = rawValues.map((value) => {
        const normalized = value ?? "";
        return rule.trim === false ? normalized : normalized.trim();
      });
      return;
    }

    case "initializeCashfreeSandbox": {
      await session.initializeCashfreeSandbox();
      return;
    }

    default: {
      const _never: never = rule;
      throw new Error(`Unsupported rule action: ${String(_never)}`);
    }
  }
}
