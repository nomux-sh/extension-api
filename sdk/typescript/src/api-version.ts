import type { ApiVersion } from "./generated/nomux/extension/v1/handshake.js";

export type { ApiVersion };

/**
 * The extension API version that this package implements. It is equal to the
 * version of the package.
 */
export const API_VERSION: Readonly<ApiVersion> = Object.freeze({
  major: 0,
  minor: 1,
  patch: 2,
});

/** Formats a version as `major.minor.patch`. */
export function formatVersion(version: ApiVersion): string {
  return `${version.major}.${version.minor}.${version.patch}`;
}

/**
 * Returns `true` if an extension that was built for `builtFor` can use a host
 * that supplies `host`. This is the caret rule of the specification: the
 * first part of `builtFor` that is not 0 sets the limit.
 */
export function isCompatible(builtFor: ApiVersion, host: ApiVersion): boolean {
  const limit =
    builtFor.major > 0
      ? [builtFor.major + 1, 0, 0]
      : builtFor.minor > 0
        ? [0, builtFor.minor + 1, 0]
        : [0, 0, builtFor.patch + 1];
  const version = [host.major, host.minor, host.patch];
  const minimum = [builtFor.major, builtFor.minor, builtFor.patch];
  return compare(minimum, version) <= 0 && compare(version, limit) < 0;
}

function compare(left: readonly number[], right: readonly number[]): number {
  for (let index = 0; index < left.length; index += 1) {
    const difference = (left[index] ?? 0) - (right[index] ?? 0);
    if (difference !== 0) {
      return difference;
    }
  }
  return 0;
}
