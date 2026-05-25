import { describe, expect, it } from "vitest";
import { Semaphore } from "../semaphore.js";

describe("Semaphore", () => {
  it("rejects permits < 1 at construction", () => {
    expect(() => new Semaphore(0)).toThrow();
    expect(() => new Semaphore(-1)).toThrow();
  });

  it("hands out up to N permits without blocking", async () => {
    const sem = new Semaphore(3);
    const r1 = await sem.acquire();
    const r2 = await sem.acquire();
    const r3 = await sem.acquire();
    expect(sem.available_permits()).toBe(0);
    r1();
    r2();
    r3();
    expect(sem.available_permits()).toBe(3);
  });

  it("queues the (N+1)th caller until a permit is released", async () => {
    const sem = new Semaphore(2);
    const r1 = await sem.acquire();
    const r2 = await sem.acquire();

    let third: (() => void) | null = null;
    const thirdAcquired = sem.acquire().then((release) => {
      third = release;
    });

    // Give the promise a tick to see if it resolves immediately (it shouldn't).
    await new Promise((r) => setImmediate(r));
    expect(third).toBeNull();

    r1();
    await thirdAcquired;
    expect(third).not.toBeNull();
    r2();
    third!();
    expect(sem.available_permits()).toBe(2);
  });

  it("hands permits to waiters in FIFO order", async () => {
    const sem = new Semaphore(1);
    const r0 = await sem.acquire();
    const order: string[] = [];
    const a = sem.acquire().then((rel) => {
      order.push("a");
      rel();
    });
    const b = sem.acquire().then((rel) => {
      order.push("b");
      rel();
    });
    const c = sem.acquire().then((rel) => {
      order.push("c");
      rel();
    });
    r0();
    await Promise.all([a, b, c]);
    expect(order).toEqual(["a", "b", "c"]);
  });

  it("a double-release is a no-op", async () => {
    const sem = new Semaphore(1);
    const release = await sem.acquire();
    release();
    release(); // second call should not bump available beyond the cap
    expect(sem.available_permits()).toBe(1);
  });
});
