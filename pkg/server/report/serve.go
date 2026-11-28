package report

import (
	"context"
	"crypto/rand"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/netip"
	"time"

	core "github.com/alswl/mind-forge/pkg/services/report"
)

const DefaultAddress = "127.0.0.1:8787"

func Serve(ctx context.Context, address string, service core.Service, scope core.Scope, startup io.Writer) error {
	if address == "" {
		address = DefaultAddress
	}
	host, port, err := net.SplitHostPort(address)
	if err != nil {
		return fmt.Errorf("listen address: %w", err)
	}
	if host == "localhost" {
		host = "127.0.0.1"
	}
	ip, err := netip.ParseAddr(host)
	if err != nil || !ip.IsLoopback() || ip.Zone() != "" {
		return fmt.Errorf("listen host %q must be loopback", host)
	}
	if err := service.Ready(scope); err != nil {
		return fmt.Errorf("report project readiness: %w", err)
	}
	listener, err := (&net.ListenConfig{}).Listen(ctx, "tcp", net.JoinHostPort(host, port))
	if err != nil {
		return err
	}
	defer listener.Close()

	options := APIOptions{Service: service, Scope: scope, Address: listener.Addr().String(), Token: rand.Text()}
	handler, _ := NewAPI(options)
	server := &http.Server{
		Handler: handler, ReadHeaderTimeout: 5 * time.Second, IdleTimeout: 30 * time.Second,
		BaseContext: func(net.Listener) context.Context { return ctx },
	}
	if _, err := fmt.Fprintf(startup, "Report API http://%s\nBearer token %s\n", options.Address, options.Token); err != nil {
		return err
	}
	shutdown := make(chan error, 1)
	stop := context.AfterFunc(ctx, func() {
		deadline, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		err := server.Shutdown(deadline)
		if err != nil {
			err = errors.Join(err, server.Close())
		}
		shutdown <- err
	})
	err = server.Serve(listener)
	if errors.Is(err, http.ErrServerClosed) {
		err = nil
	}
	if !stop() {
		err = errors.Join(err, <-shutdown)
	}
	return err
}
