package task

import (
	"context"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestOptionalObserverIsSynchronousAndInvocationLocal(t *testing.T) {
	var first, second []Stage
	a := WithObserver(context.Background(), func(stage Stage) { first = append(first, stage) })
	b := WithObserver(a, func(stage Stage) { second = append(second, stage) })
	event := Stage{Operation: "build", Name: "rendering"}
	Notify(a, event)
	require.Equal(t, []Stage{event}, first)
	require.Empty(t, second)
	Notify(b, Stage{Operation: "source", Name: "reading"})
	require.Len(t, first, 1)
	require.Len(t, second, 1)
	Notify(WithObserver(a, nil), event)
	Notify(context.Background(), event)
	require.Len(t, first, 1)
	ctx, cancel := context.WithCancel(a)
	cancel()
	Notify(ctx, event)
	require.Len(t, first, 1)
}
