package extension

import (
	"fmt"

	"github.com/nomux-sh/extension-api/sdk/go/internal/wire"
)

// Version is a version of the extension API. The extension API uses Semantic
// Versioning 2.0.0, and its version is independent of the product version of
// nomux.
type Version struct {
	Major uint32
	Minor uint32
	Patch uint32
}

// APIVersion returns the extension API version that this module implements.
// It is equal to the version of the module.
func APIVersion() Version {
	return Version{Major: 0, Minor: 1, Patch: 1}
}

// String formats the version as major.minor.patch.
func (v Version) String() string {
	return fmt.Sprintf("%d.%d.%d", v.Major, v.Minor, v.Patch)
}

// accepts reports whether an extension that was built for v can use a host
// that supplies host. This is the caret rule of the specification: the first
// part of v that is not 0 sets the limit.
func (v Version) accepts(host Version) bool {
	var limit Version
	switch {
	case v.Major > 0:
		limit = Version{Major: v.Major + 1}
	case v.Minor > 0:
		limit = Version{Minor: v.Minor + 1}
	default:
		limit = Version{Patch: v.Patch + 1}
	}
	return !host.less(v) && host.less(limit)
}

func (v Version) less(other Version) bool {
	if v.Major != other.Major {
		return v.Major < other.Major
	}
	if v.Minor != other.Minor {
		return v.Minor < other.Minor
	}
	return v.Patch < other.Patch
}

func versionFromWire(version *wire.ApiVersion) Version {
	return Version{
		Major: version.GetMajor(),
		Minor: version.GetMinor(),
		Patch: version.GetPatch(),
	}
}

func (v Version) toWire() *wire.ApiVersion {
	return &wire.ApiVersion{Major: v.Major, Minor: v.Minor, Patch: v.Patch}
}
