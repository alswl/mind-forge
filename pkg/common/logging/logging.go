package logging

import (
	"context"
	"io"
	"log/slog"
)

type loggerKey struct{}

// WithLogger keeps operational logging local to one execution, including tests.
func WithLogger(ctx context.Context, stderr io.Writer, verbose bool) context.Context {
	level := slog.LevelWarn
	if verbose {
		level = slog.LevelDebug
	}
	logger := slog.New(slog.NewTextHandler(stderr, &slog.HandlerOptions{Level: level}))
	return context.WithValue(ctx, loggerKey{}, logger)
}

func FromContext(ctx context.Context) *slog.Logger {
	if logger, ok := ctx.Value(loggerKey{}).(*slog.Logger); ok {
		return logger
	}
	return slog.New(slog.NewTextHandler(io.Discard, nil))
}
