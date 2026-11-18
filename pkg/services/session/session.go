package session

import (
	"context"
	"io"
	"strings"

	"github.com/alswl/mind-forge/pkg/dal/store"
	"github.com/alswl/mind-forge/pkg/managers/lifecycle"
)

func Begin(ctx context.Context, root string, write, recovery bool) (io.Closer, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	if root == "" || recovery {
		return io.NopCloser(strings.NewReader("")), nil
	}
	var lock io.Closer = io.NopCloser(strings.NewReader(""))
	if write {
		held, err := store.AcquireWriteLock(root)
		if err != nil {
			return nil, err
		}
		lock = held
	}
	if err := (lifecycle.Engine{}).CheckRecovery(root); err != nil {
		_ = lock.Close()
		return nil, err
	}
	return lock, nil
}
