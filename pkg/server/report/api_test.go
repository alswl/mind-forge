package report

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	core "github.com/alswl/mind-forge/pkg/services/report"
	"github.com/danielgtaylor/huma/v2/humatest"
	"github.com/stretchr/testify/require"
)

type fixtureWorker struct{ called int }

func (worker *fixtureWorker) Run(_ context.Context, job core.Job) (core.Receipt, error) {
	worker.called++
	err := os.WriteFile(job.Stage, []byte("synthetic "+job.View), 0600)
	return core.Receipt{Protocol: job.Protocol, Stage: job.Stage, Result: core.Result{
		Protocol: job.Protocol, Renderer: job.Renderer.ID, RendererVersion: job.Renderer.Version,
		View: job.View, Artifact: job.Artifact,
	}}, err
}

func TestHumaCoreAPI(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "weekly")
	require.NoError(t, os.Mkdir(dir, 0755))
	for name, data := range map[string]string{
		"report.yaml": "apiVersion: report.mind-forge/v1\nkind: Report\nrenderer: builtin/brief\nentry: report.tsx\ndata: data.json\n",
		"report.tsx":  "// fixture entry",
		"data.json":   `{"title":"Synthetic"}`,
	} {
		require.NoError(t, os.WriteFile(filepath.Join(dir, name), []byte(data), 0644))
	}
	scope, err := core.StandaloneScope(dir)
	require.NoError(t, err)
	const host = "127.0.0.1:8787"
	const token = "local-test-token"
	const path = "/v1/reports/weekly/exports"
	worker := &fixtureWorker{}
	handler, api := NewAPI(APIOptions{Service: core.Service{Worker: worker}, Scope: scope, Address: host, Token: token})
	send := func(method, url, requestHost, origin, auth, body string) *httptest.ResponseRecorder {
		t.Helper()
		request := httptest.NewRequestWithContext(t.Context(), method, url, strings.NewReader(body))
		request.Host = requestHost
		if body != "" {
			request.Header.Set("Content-Type", "application/json")
		}
		if origin != "" {
			request.Header.Set("Origin", origin)
		}
		if auth != "" {
			request.Header.Set("Authorization", "Bearer "+auth)
		}
		response := httptest.NewRecorder()
		handler.ServeHTTP(response, request)
		return response
	}

	humaTest := humatest.Wrap(t, api)
	for _, url := range []string{"/health", "/ready", "/docs", "/openapi.yaml", "/v1/renderers?id=builtin%2Fbrief", "/v1/components"} {
		response := humaTest.GetCtx(t.Context(), "http://"+host+url)
		require.Equal(t, 200, response.Code, "%s: %s", url, response.Body.String())
	}
	response := send(http.MethodGet, "/v1/renderers?id=repo%2Fmissing", host, "", "", "")
	requireProblem(t, response, 404)
	for _, tc := range []struct {
		path, host, origin, auth, body string
		code                           int
	}{
		{path, host, "", "", "{}", 401},
		{path, host, "", "wrong", "{}", 401},
		{path, "example.invalid:8787", "", token, "{}", 403},
		{path, host, "null", token, "{}", 403},
		{path, host, "http://localhost:8787", token, "{}", 403},
		{path, host, "", token, `{"view":"unknown"}`, 422},
		{path, host, "", token, `{"entry":"report.tsx"}`, 422},
		{path, host, "", token, `{"renderer":"repo/missing"}`, 404},
		{"/v1/reports/unknown/exports", host, "", token, "{}", 404},
	} {
		requireProblem(t, send(http.MethodPost, tc.path, tc.host, tc.origin, tc.auth, tc.body), tc.code)
	}
	require.Zero(t, worker.called)

	response = send(http.MethodPost, path, host, "http://"+host, token, `{"view":"table","theme":"dark"}`)
	require.Equal(t, 200, response.Code, response.Body.String())
	var result core.Result
	require.NoError(t, json.Unmarshal(response.Body.Bytes(), &result))
	require.Equal(t, "outputs/report.table.html", result.Artifact)
	require.NotContains(t, response.Body.String(), "$schema")
	require.Equal(t, 1, worker.called)
	response = send(http.MethodPost, path, host, "", token, "")
	require.Equal(t, 200, response.Code, response.Body.String())
	require.Equal(t, 2, worker.called)

	require.NoError(t, os.WriteFile(filepath.Join(dir, "data.json"), []byte(`{"title":12}`), 0644))
	requireProblem(t, send(http.MethodPost, path, host, "", token, "{}"), 422)
	require.Equal(t, 2, worker.called)

	require.NoError(t, os.WriteFile(filepath.Join(dir, "data.json"), []byte(`{"title":"Synthetic"}`), 0644))
	t.Setenv("PATH", t.TempDir())
	handler, _ = NewAPI(APIOptions{Service: core.Service{Worker: core.ProcessWorker{Stderr: io.Discard}}, Scope: scope, Address: host, Token: token})
	missingNode := send(http.MethodPost, path, host, "", token, `{"view":"table"}`)
	requireProblem(t, missingNode, 503)
	require.Contains(t, missingNode.Body.String(), "node executable not found on PATH")
	output, err := os.ReadFile(filepath.Join(dir, "outputs/report.table.html"))
	require.NoError(t, err)
	require.Equal(t, "synthetic table", string(output))

	raw, err := OpenAPI()
	require.NoError(t, err)
	require.Contains(t, string(raw), "openapi: 3.1.0")
	require.Contains(t, string(raw), "localBearer")
	require.Contains(t, string(raw), "/v1/reports/{slug}/exports")
}

func requireProblem(t *testing.T, response *httptest.ResponseRecorder, code int) {
	t.Helper()
	require.Equal(t, code, response.Code, response.Body.String())
	require.Contains(t, response.Header().Get("Content-Type"), "application/problem+json")
	require.Contains(t, response.Body.String(), "\"status\":")
}
