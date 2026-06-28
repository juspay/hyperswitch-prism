// Inject an external <script> once and resolve when it has loaded. Used for
// third-party widgets that ship only as a script (e.g. Razorpay Checkout JS),
// unlike Stripe/Adyen which are npm packages.
const loaded = new Map<string, Promise<void>>();

export function loadScript(src: string): Promise<void> {
  const existing = loaded.get(src);
  if (existing) return existing;

  const p = new Promise<void>((resolve, reject) => {
    const el = document.createElement("script");
    el.src = src;
    el.async = true;
    el.onload = () => resolve();
    el.onerror = () => reject(new Error(`Failed to load script: ${src}`));
    document.head.appendChild(el);
  });

  loaded.set(src, p);
  return p;
}
