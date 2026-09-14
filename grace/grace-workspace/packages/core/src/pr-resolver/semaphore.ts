/**
 * Promise-based counting semaphore. Used by the PR Resolver to cap the
 * number of PR workers running concurrently — `acquire()` resolves when a
 * permit is free, the returned `release` MUST be called when the worker
 * finishes (or rejects/throws).
 *
 * No fairness guarantees; FIFO via the JS event loop's task ordering is
 * close enough for our scale (N ≤ 16). Single-threaded JS means no real
 * mutex needed inside; the `permits` counter and waiter queue are touched
 * only between awaits.
 */
export class Semaphore {
  private available: number;
  private waiters: Array<() => void> = [];

  constructor(permits: number) {
    if (permits < 1) {
      throw new Error(`Semaphore: permits must be >= 1, got ${permits}`);
    }
    this.available = permits;
  }

  async acquire(): Promise<() => void> {
    if (this.available > 0) {
      this.available -= 1;
      return this.makeRelease();
    }
    await new Promise<void>((resolve) => this.waiters.push(resolve));
    return this.makeRelease();
  }

  /** Snapshot of permits not currently held. Useful for logs/metrics. */
  available_permits(): number {
    return this.available;
  }

  private makeRelease(): () => void {
    let released = false;
    return () => {
      if (released) return;
      released = true;
      const next = this.waiters.shift();
      if (next) {
        // Hand the permit straight to the next waiter without flipping
        // through `available` — keeps the count consistent and avoids a
        // brief window where a fresh acquire could jump the queue.
        next();
      } else {
        this.available += 1;
      }
    };
  }
}
