//go:build !mf_testfaults

package cli

import "github.com/alswl/mind-forge/internal/lifecycle"

func lifecycleEngine() lifecycle.Engine { return lifecycle.Engine{} }
