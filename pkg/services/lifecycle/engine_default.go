//go:build !mf_testfaults

package lifecycle

import manager "github.com/alswl/mind-forge/pkg/managers/lifecycle"

func newEngine() manager.Engine { return manager.Engine{} }
