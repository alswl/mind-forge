package reportcli

import (
	"errors"
	"fmt"
	"io"
	"path/filepath"

	"github.com/alswl/mind-forge/internal/output"
	server "github.com/alswl/mind-forge/pkg/server/report"
	core "github.com/alswl/mind-forge/pkg/services/report"
	"github.com/spf13/cobra"
)

func Service(registry, worker string, stderr io.Writer) (core.Service, error) {
	root, err := core.RegistryRoot(registry)
	if err != nil {
		return core.Service{}, output.Usage(err.Error(), "select the directory containing minds.yaml")
	}
	if worker != "" {
		worker, err = filepath.Abs(worker)
		if err != nil {
			return core.Service{}, err
		}
	}
	return core.Service{Registry: root, Worker: core.ProcessWorker{Executable: worker, Stderr: stderr}}, nil
}

func Build(command *cobra.Command, service core.Service, scope core.Scope, request core.Request, dryRun bool) (any, error) {
	if dryRun {
		return service.Plan(command.Context(), scope, request)
	}
	return service.Build(command.Context(), scope, request)
}

func OpenAPI(writer io.Writer, project bool) error {
	raw, err := server.Schema(project)
	if err != nil {
		return err
	}
	_, err = writer.Write(raw)
	return err
}

func Result(writer io.Writer, name, format string, quiet bool, value any, err error) error {
	if err != nil {
		switch {
		case errors.Is(err, core.ErrInvalid):
			return &output.Error{Kind: "incompatible_schema", Message: err.Error(), Cause: err}
		case errors.Is(err, core.ErrConflict):
			return &output.Error{Kind: "content_conflict", Message: err.Error(), Cause: err}
		case errors.Is(err, core.ErrNotFound):
			return &output.Error{Kind: "not_found", Message: err.Error(), Cause: err}
		case errors.Is(err, core.ErrUnavailable):
			return &output.Error{Kind: "worker_unavailable", Message: err.Error(), Cause: err}
		default:
			return err
		}
	}
	if quiet {
		return nil
	}
	if format == "json" {
		raw, err := output.Success(name, value)
		if err != nil {
			return err
		}
		_, err = writer.Write(raw)
		return err
	}
	if catalog, ok := value.(core.Catalog); ok {
		for _, item := range catalog.Items {
			if _, err := fmt.Fprintf(writer, "%s\t%s\t%s\n", item.ID, item.Version, item.Source); err != nil {
				return err
			}
		}
		for _, diagnostic := range catalog.Diagnostics {
			if _, err := fmt.Fprintf(writer, "%s\t%s\n", diagnostic.Path, diagnostic.Message); err != nil {
				return err
			}
		}
		return nil
	}
	return output.RenderSuccess(writer, format, value)
}
