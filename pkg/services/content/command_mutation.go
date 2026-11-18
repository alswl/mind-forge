package content

import (
	"context"
	"io"
	"path/filepath"
	"strings"

	"github.com/alswl/mind-forge/pkg/dal/store"
	"github.com/alswl/mind-forge/pkg/managers/lifecycle"
)

func ValidateArticleArgument(project, input string) error {
	if err := store.RejectTraversal(input, "project"); err != nil {
		return err
	}
	if strings.ContainsAny(input, "/\\") {
		target := input
		if !filepath.IsAbs(target) {
			target = filepath.Join(project, input)
		}
		_, err := store.CanonicalWithin(project, target)
		return err
	}
	return nil
}

// Run owns the root write lock for these mutations. The engine still checks
// its journal and inventory, borrowing that lock instead of acquiring it twice.
func ApplyMutation(ctx context.Context, root string, mutation ContentMutation) error {
	engine := lifecycleWithHeldLock()
	engine.Context = ctx
	_, err := engine.Apply(root, mutation.Plan)
	return err
}

func lifecycleWithHeldLock() lifecycle.Engine {
	return lifecycle.Engine{AcquireLock: func(string) (io.Closer, error) { return io.NopCloser(strings.NewReader("")), nil }}
}
