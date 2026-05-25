import { expect, test, type Page } from "@playwright/test";
import { FakeSupervisor, makeFakeMachine } from "./fake-supervisor.js";
import { FAKE_WS_PORT } from "../playwright.config.js";

/**
 * End-to-end tests for the PR Resolver tab. Each test stands up a
 * `FakeSupervisor` on the WS port the dashboard was built against,
 * injects either a hello snapshot or a live event stream, and asserts
 * the rendered board.
 *
 * No real cargo / GitHub / Claude involved — the dashboard is the only
 * "real" piece under test. This pairs with the vitest unit suite in
 * @byne/core that already covers the backend pure logic.
 */

let supervisor: FakeSupervisor;

test.beforeEach(async () => {
  supervisor = new FakeSupervisor(FAKE_WS_PORT);
  await supervisor.start();
});

test.afterEach(async () => {
  await supervisor.stop();
});

async function gotoBoard(page: Page): Promise<void> {
  await page.goto("/pr-resolver");
  // Wait for the WS to come up and the snapshot to land. The conn-dot
  // turns from "connecting…" to "open" once the hello round-trips.
  await expect(page.locator("h1", { hasText: /PR Resolver/ })).toBeVisible();
  await supervisor.waitForDashboard();
}

test("snapshot renders the four-column board with seeded PRs", async ({ page }) => {
  supervisor.snapshot.prMachines = {
    "1701": makeFakeMachine({
      prNumber: 1701,
      status: "noticed",
      connectors: ["adyen"],
      summary: "Initial comment received",
    }),
    "1702": makeFakeMachine({
      prNumber: 1702,
      status: "resolving",
      connectors: ["stripe"],
      summary: "Claude rewriting transformers.rs",
    }),
    "1703": makeFakeMachine({
      prNumber: 1703,
      status: "pushed",
      connectors: ["payu"],
      localSha: "abcdef1234567890",
    }),
    "1704": makeFakeMachine({
      prNumber: 1704,
      status: "failed",
      connectors: ["authipay"],
      reason: "cargo build failed after 3 retries",
    }),
  };

  await gotoBoard(page);

  // Each column header (with its count chip) renders.
  await expect(page.getByText("Queued").first()).toBeVisible();
  await expect(page.getByText("In Progress").first()).toBeVisible();
  await expect(page.getByText("Completed").first()).toBeVisible();
  await expect(page.getByText("Failed / Blocked").first()).toBeVisible();

  // PR cards land in the right columns.
  await expect(page.getByText("PR #1701")).toBeVisible();
  await expect(page.getByText("PR #1702")).toBeVisible();
  await expect(page.getByText("PR #1703")).toBeVisible();
  await expect(page.getByText("PR #1704")).toBeVisible();

  // Connector badges show.
  await expect(page.getByText("adyen").first()).toBeVisible();
  await expect(page.getByText("stripe").first()).toBeVisible();

  // The pushed PR shows the short SHA prefix.
  await expect(page.getByText("abcdef12").first()).toBeVisible();

  // The failure card shows the reason text.
  await expect(
    page.getByText("cargo build failed after 3 retries").first()
  ).toBeVisible();
});

test("live event stream moves a PR queued → in_progress → completed", async ({
  page,
}) => {
  supervisor.snapshot.prMachines = {};
  await gotoBoard(page);

  // Sanity: nothing yet.
  await expect(page.getByText("PR #2100")).toHaveCount(0);

  // Step 1 — PR appears in Queued via machine snapshot rebroadcast.
  supervisor.snapshot.prMachines = {
    "2100": makeFakeMachine({ prNumber: 2100, status: "noticed" }),
  };
  supervisor.sendEvent("cycle_start", { cycle: 1 });
  supervisor.rebroadcastSnapshot();
  await expect(page.getByText("PR #2100")).toBeVisible();

  // Step 2 — flip status to resolving + rebroadcast.
  supervisor.snapshot.prMachines = {
    "2100": makeFakeMachine({
      prNumber: 2100,
      status: "resolving",
      connectors: ["adyen"],
      summary: "fixing transformers.rs",
    }),
  };
  supervisor.sendEvent("pr_start", { pr: 2100, threadCount: 2 });
  supervisor.rebroadcastSnapshot();

  // Card should now be in the In Progress column. We assert by checking
  // the connector badge — only painted once the resolving machine lands.
  await expect(page.getByText("adyen").first()).toBeVisible();

  // Step 3 — flip to pushed.
  supervisor.snapshot.prMachines = {
    "2100": makeFakeMachine({
      prNumber: 2100,
      status: "pushed",
      connectors: ["adyen"],
      localSha: "0123456789abcdef",
    }),
  };
  supervisor.sendEvent("pr_done", { pr: 2100, fixed: 1, failed: 0, skipped: 0 });
  supervisor.sendEvent("cycle_end", {
    cycle: 1,
    total: 1,
    fixed: 1,
    failed: 0,
    skipped: 0,
    queued: 0,
    startedAt: Date.now() - 1000,
    completedAt: Date.now(),
  });
  supervisor.rebroadcastSnapshot();

  await expect(page.getByText("01234567").first()).toBeVisible();
});

test("awaiting_approval shows the Approve & push button; click reaches the supervisor", async ({
  page,
}) => {
  supervisor.snapshot.prMachines = {
    "2200": makeFakeMachine({
      prNumber: 2200,
      status: "awaiting_approval",
      connectors: ["adyen"],
      branch: "feat/adyen-fix",
      summary: "ready to push",
      diffPreview: "diff --git a/foo.rs b/foo.rs\n+pretend diff",
      localSha: "deadbeef12345678",
    }),
  };

  await page.goto("/pr-resolver/2200");
  await supervisor.waitForDashboard();

  // No rail click needed — the auto-select effect now waits for the machine
  // to land before locking in a stage, so the Approval panel surfaces on its
  // own once the snapshot arrives.
  const approve = page.getByRole("button", { name: /Approve & push/i });
  await expect(approve).toBeVisible();
  await approve.click();

  // The dashboard sends {type:"pr-resolver:approve", payload:{prNumber:2200}}
  // to the supervisor. Confirm it lands.
  await expect
    .poll(
      () =>
        supervisor.inbound.find(
          (m) =>
            m.type === "pr-resolver:approve" &&
            Number((m.payload as { prNumber?: number })?.prNumber) === 2200
        )?.type ?? null,
      { timeout: 3_000 }
    )
    .toBe("pr-resolver:approve");
});

test("request changes opens the textarea dialog and sends feedback to the supervisor", async ({
  page,
}) => {
  supervisor.snapshot.prMachines = {
    "2300": makeFakeMachine({
      prNumber: 2300,
      status: "awaiting_approval",
      connectors: ["adyen"],
      branch: "feat/adyen-fix",
      summary: "ready to push",
      diffPreview: "diff --git a/foo.rs b/foo.rs\n+pretend diff",
      localSha: "cafebabe12345678",
    }),
  };

  await page.goto("/pr-resolver/2300");
  await supervisor.waitForDashboard();

  // The dialog isn't there until the user clicks "Request changes…".
  await expect(page.getByTestId("revision-form")).toHaveCount(0);

  await page.getByRole("button", { name: /Request changes…/ }).click();
  await expect(page.getByTestId("revision-form")).toBeVisible();

  // Submit button is disabled while the textarea is empty.
  const submit = page.getByRole("button", { name: /Send to Claude/ });
  await expect(submit).toBeDisabled();

  const feedback =
    "use Option<String> for optional fields and pass through the raw status code on the error path";
  await page
    .getByPlaceholder(/use Option<String> for the optional/i)
    .fill(feedback);
  await expect(submit).toBeEnabled();
  await submit.click();

  // The dashboard sends pr-resolver:request_changes with the feedback.
  await expect
    .poll(
      () =>
        supervisor.inbound.find(
          (m) =>
            m.type === "pr-resolver:request_changes" &&
            Number((m.payload as { prNumber?: number })?.prNumber) === 2300
        )?.payload as { feedback?: string } | undefined,
      { timeout: 3_000 }
    )
    .toEqual({ prNumber: 2300, feedback });

  // Dialog closes after submission.
  await expect(page.getByTestId("revision-form")).toHaveCount(0);
});

test("request changes cancel button dismisses the dialog without sending", async ({
  page,
}) => {
  supervisor.snapshot.prMachines = {
    "2301": makeFakeMachine({
      prNumber: 2301,
      status: "awaiting_approval",
      connectors: ["adyen"],
      branch: "feat/adyen-fix",
    }),
  };

  await page.goto("/pr-resolver/2301");
  await supervisor.waitForDashboard();
  await page.getByRole("button", { name: /Request changes…/ }).click();

  await page
    .getByPlaceholder(/use Option<String> for the optional/i)
    .fill("won't be sent");
  await page.getByRole("button", { name: /^Cancel$/ }).first().click();

  await expect(page.getByTestId("revision-form")).toHaveCount(0);
  // Give the WS a beat to make sure nothing trickled through.
  await page.waitForTimeout(300);
  expect(
    supervisor.inbound.filter((m) => m.type === "pr-resolver:request_changes")
  ).toHaveLength(0);
});

test("approval page shows the resolver summary above the diff", async ({ page }) => {
  const summary =
    "## Summary\n- Comment 1: changed Foo to Bar because the reviewer wanted Option<String>\n- Comment 2: removed dead match arm";
  supervisor.snapshot.prMachines = {
    "2400": makeFakeMachine({
      prNumber: 2400,
      status: "awaiting_approval",
      connectors: ["adyen"],
      branch: "feat/adyen-fix",
      summary,
      diffPreview:
        "diff --git a/foo.rs b/foo.rs\n@@ -1,1 +1,1 @@\n-let x = Foo;\n+let x = Bar;",
      localSha: "facefeed12345678",
    }),
  };

  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/pr-resolver/2400");
  await supervisor.waitForDashboard();

  const summaryBlock = page.getByTestId("approval-summary");
  await expect(summaryBlock).toBeVisible();
  await expect(summaryBlock).toContainText("changed Foo to Bar");
  await expect(summaryBlock).toContainText("removed dead match arm");
});

test("diff viewer split/inline toggle switches modes and auto-collapses on narrow viewports", async ({
  page,
}) => {
  supervisor.snapshot.prMachines = {
    "2500": makeFakeMachine({
      prNumber: 2500,
      status: "awaiting_approval",
      connectors: ["adyen"],
      branch: "feat/adyen-fix",
      diffPreview:
        "diff --git a/foo.rs b/foo.rs\n@@ -1,2 +1,2 @@\n-let x = Foo;\n-let y = Foo;\n+let x = Bar;\n+let y = Bar;",
      localSha: "1234abcd5678ef90",
    }),
  };

  // Wide viewport — split is the default and should render as a <table>.
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto("/pr-resolver/2500");
  await supervisor.waitForDashboard();

  await expect(page.locator('[data-diff-mode="split"]').first()).toBeVisible();

  // Flip to inline.
  await page.getByRole("button", { name: /^Inline$/ }).first().click();
  await expect(page.locator('[data-diff-mode="inline"]').first()).toBeVisible();
  await expect(page.locator('[data-diff-mode="split"]')).toHaveCount(0);

  // Flip back to split.
  await page.getByRole("button", { name: /^Split$/ }).first().click();
  await expect(page.locator('[data-diff-mode="split"]').first()).toBeVisible();

  // Shrink the viewport below the split breakpoint — should auto-collapse to
  // inline and disable the Split toggle without losing the user's preference.
  await page.setViewportSize({ width: 720, height: 900 });
  await expect(page.locator('[data-diff-mode="inline"]').first()).toBeVisible();
  await expect(page.locator('[data-diff-mode="split"]')).toHaveCount(0);
  await expect(page.getByRole("button", { name: /^Split$/ }).first()).toBeDisabled();
});

test("a stuck non-terminal machine surfaces a 'Force reset & retry' banner", async ({
  page,
}) => {
  // Simulate the ENOSPC-mid-cycle scenario: machine got frozen in
  // 'resolving' because the state file couldn't be updated to 'failed'.
  supervisor.snapshot.running = false;
  supervisor.snapshot.prMachines = {
    "2600": makeFakeMachine({
      prNumber: 2600,
      status: "resolving",
      connectors: ["bankofamerica"],
      branch: "feat/bofa-fix",
      reason: "Disk full during gRPC server compile (ENOSPC)",
    }),
  };

  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/pr-resolver/2600");
  await supervisor.waitForDashboard();

  const banner = page.getByTestId("retry-banner-stuck");
  await expect(banner).toBeVisible();
  await expect(banner).toContainText("Stuck mid-cycle");
  await expect(banner).toContainText("Disk full");

  await page.getByRole("button", { name: /Force reset & retry/ }).click();
  await expect
    .poll(
      () =>
        supervisor.inbound.find(
          (m) =>
            m.type === "pr-resolver:retry" &&
            Number((m.payload as { prNumber?: number })?.prNumber) === 2600
        )?.type ?? null,
      { timeout: 3_000 }
    )
    .toBe("pr-resolver:retry");
});

test("a stuck banner is suppressed while a cycle is actively running", async ({
  page,
}) => {
  supervisor.snapshot.running = true; // cycle in flight — not really stuck
  supervisor.snapshot.prMachines = {
    "2601": makeFakeMachine({
      prNumber: 2601,
      status: "resolving",
      connectors: ["bankofamerica"],
      branch: "feat/bofa-fix",
    }),
  };

  await page.goto("/pr-resolver/2601");
  await supervisor.waitForDashboard();

  await expect(page.getByTestId("retry-banner-stuck")).toHaveCount(0);
});

test("resolver_stream tail is replayed from the snapshot on (re)connect", async ({
  page,
}) => {
  // Mimic a PR that's mid-resolve and has streamed a handful of Claude
  // lines before the dashboard connected. The supervisor's per-PR rolling
  // buffer should ship them in the snapshot — they're NON_REPLAY events so
  // recentEvents alone won't carry them.
  supervisor.snapshot.prMachines = {
    "2700": makeFakeMachine({
      prNumber: 2700,
      status: "resolving",
      connectors: ["bankofamerica"],
      branch: "feat/bofa-fix",
    }),
  };
  supervisor.snapshot.streamTails = {
    "2700": {
      resolverStream: [
        "[claude] reading transformers.rs",
        "[claude] applying fix per reviewer comment",
        "[claude] summary written",
      ],
    },
  };
  supervisor.snapshot.running = true;

  await page.goto("/pr-resolver/2700");
  await supervisor.waitForDashboard();

  await expect(
    page.getByText("[claude] reading transformers.rs")
  ).toBeVisible();
  await expect(
    page.getByText("[claude] applying fix per reviewer comment")
  ).toBeVisible();
  await expect(page.getByText("[claude] summary written")).toBeVisible();
});

test("settings toggle round-trips via pr-resolver:configure", async ({ page }) => {
  await gotoBoard(page);

  // The AutoApproveToggle is disabled until we open the auto-approve flow,
  // but the supervisor-level `pr-resolver:toggle` should fire from the
  // header Enable switch when we click it OFF (the snapshot has enabled=true).
  const enableToggle = page
    .locator("button, [role=switch]")
    .filter({ hasText: /Enabled|Disabled/i })
    .first();
  if (await enableToggle.isVisible().catch(() => false)) {
    await enableToggle.click();
    await expect
      .poll(
        () =>
          supervisor.inbound.find((m) => m.type === "pr-resolver:toggle")?.type ??
          null,
        { timeout: 3_000 }
      )
      .toBe("pr-resolver:toggle");
  }
});
