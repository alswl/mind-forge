package task

import "context"

// Context supports callers that do not initiate a cancellable operation.
func Context(ctx context.Context) context.Context {
	if ctx == nil {
		return context.Background()
	}
	return ctx
}
