export const DEFAULT_WS_PORT = "3142";

export const CONTROL_WS_PORT: string =
  (import.meta.env.VITE_WS_PORT as string | undefined) ?? DEFAULT_WS_PORT;
