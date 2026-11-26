import type { Host } from "./host.js";

/** The context of one call. Each method gets it as its second argument. */
export interface Call {
  /** The host that sent the call. */
  readonly host: Host;
  /**
   * Aborted when the host cancels the call. A method that can run for a long
   * time gives it to the functions that it calls, or examines
   * `signal.aborted` between its steps.
   */
  readonly signal: AbortSignal;
}
