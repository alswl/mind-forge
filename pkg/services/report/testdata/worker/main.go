// The controlled worker exercises the process contract without a Node runtime.
package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"html"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/alswl/mind-forge/pkg/services/report"
)

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func run() error {
	decoder := json.NewDecoder(os.Stdin)
	decoder.DisallowUnknownFields()
	var job report.Job
	if err := decoder.Decode(&job); err != nil {
		return err
	}
	var extra any
	if err := decoder.Decode(&extra); !errors.Is(err, io.EOF) {
		return fmt.Errorf("expected one request")
	}
	if job.Protocol != report.Protocol {
		return fmt.Errorf("unsupported protocol %q", job.Protocol)
	}
	cwd, err := os.Getwd()
	if err != nil {
		return err
	}
	if cwd != job.WorkDir || !filepath.IsAbs(job.Entry) || !filepath.IsAbs(job.Data) ||
		filepath.Dir(job.Stage) != filepath.Join(job.WorkDir, "outputs") || !json.Valid(job.Renderer.Schema) {
		return fmt.Errorf("invalid worker request paths or renderer schema")
	}
	fields := []string{job.Renderer.ID, job.View, job.Theme}
	for _, component := range job.Components {
		if !json.Valid(component.Schema) {
			return fmt.Errorf("invalid component schema")
		}
		fields = append(fields, component.ID)
	}
	body, err := os.ReadFile(job.Data)
	if err != nil {
		return err
	}
	page := "<!doctype html><title>Report fixture</title><p>" + html.EscapeString(strings.Join(fields, " ")) +
		"</p><pre>" + html.EscapeString(strings.TrimSpace(string(body))) + "</pre>\n"
	if err := os.WriteFile(job.Stage, []byte(page), 0600); err != nil {
		return err
	}

	receipt := report.Receipt{
		Protocol: job.Protocol, Stage: job.Stage,
		Result: report.Result{Protocol: job.Protocol, Renderer: job.Renderer.ID,
			RendererVersion: job.Renderer.Version, View: job.View, Artifact: job.Artifact},
	}
	switch os.Getenv("MF_REPORT_FIXTURE_MODE") {
	case "":
	case "fail":
		return fmt.Errorf("controlled failure after staging")
	case "bad-protocol":
		receipt.Protocol = "report.mind-forge/v2"
	case "bad-artifact":
		receipt.Result.Artifact = "outputs/unexpected.html"
	case "extra-stdout":
		fmt.Println("unexpected stdout")
	case "wait":
		fmt.Fprintln(os.Stderr, "fixture staged")
		time.Sleep(time.Minute)
	default:
		return fmt.Errorf("unknown fixture mode")
	}
	return json.NewEncoder(os.Stdout).Encode(receipt)
}
