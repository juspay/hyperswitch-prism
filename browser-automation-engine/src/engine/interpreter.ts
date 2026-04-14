import fs from "node:fs/promises";
import path from "node:path";
import type { BrowserSession } from "../drivers/browserDriver";
import type { Rule } from "../types/dsl";

export interface InterpreterContext {
  requestUrl: string;
  stepIndex: number;
  data: Record<string, unknown>;
  screenshotDir: string;
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

    case "screenshot": {
      const targetPath =
        rule.path ?? path.join(ctx.screenshotDir, `step-${ctx.stepIndex}-${Date.now()}.png`);

      await fs.mkdir(path.dirname(targetPath), { recursive: true });
      await session.screenshot(targetPath, {
        fullPage: rule.fullPage
      });

      if (rule.as) {
        ctx.data[rule.as] = targetPath;
      }
      return;
    }

    case "evaluate": {
      // SECURITY: This executes arbitrary JavaScript in the browser context.
      // Ensure this API endpoint is protected with authentication/authorization
      // if the server is exposed to untrusted networks.
      const result = await session.evaluate(rule.expression);
      if (rule.as) {
        ctx.data[rule.as] = result;
      }
      return;
    }

    default: {
      const _never: never = rule;
      throw new Error(`Unsupported rule action: ${String(_never)}`);
    }
  }
}
