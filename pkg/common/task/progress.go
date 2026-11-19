package task

import "context"

type Stage struct {
	Operation string
	Name      string
	Label     string
}

type Observer func(Stage)
type observerKey struct{}

func WithObserver(ctx context.Context, observer Observer) context.Context {
	return context.WithValue(Context(ctx), observerKey{}, observer)
}

// Notify is synchronous and optional, so a disabled observer adds no background
// work and cannot outlive the operation's context.
func Notify(ctx context.Context, stage Stage) {
	ctx = Context(ctx)
	if ctx.Err() != nil {
		return
	}
	if observer, ok := ctx.Value(observerKey{}).(Observer); ok && observer != nil {
		observer(stage)
	}
}
