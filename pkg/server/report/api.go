package report

import (
	"context"
	"crypto/subtle"
	"errors"
	"net"
	"net/http"
	"strings"

	core "github.com/alswl/mind-forge/pkg/services/report"
	"github.com/danielgtaylor/huma/v2"
	"github.com/danielgtaylor/huma/v2/adapters/humachi"
	"github.com/go-chi/chi/v5"
)

type APIOptions struct {
	Service core.Service
	Scope   core.Scope
	Address string
	Token   string
}

type catalogInput struct {
	ID string `query:"id" maxLength:"108" pattern:"^(builtin|repo)/[a-z0-9]+(-[a-z0-9]+)*$"`
}

type catalogOutput struct {
	Body core.Catalog
}

type exportOptions struct {
	Renderer string `json:"renderer,omitempty" maxLength:"108" pattern:"^(builtin|repo)/[a-z0-9]+(-[a-z0-9]+)*$"`
	View     string `json:"view,omitempty" enum:"chart,table"`
	Theme    string `json:"theme,omitempty" enum:"system,light,dark"`
}

type exportInput struct {
	Slug string `path:"slug" maxLength:"100" pattern:"^[a-z0-9]+(-[a-z0-9]+)*$"`
	Body *exportOptions
}

type fragmentInput struct {
	Slug     string `path:"slug" maxLength:"100" pattern:"^[a-z0-9]+(-[a-z0-9]+)*$"`
	Fragment string `path:"fragment" maxLength:"100" pattern:"^[a-z0-9]+(-[a-z0-9]+)*$"`
}

type exportOutput struct {
	Body core.Result
}

type statusOutput struct {
	Body struct {
		Status string `json:"status"`
	}
}

func NewAPI(options APIOptions) (http.Handler, huma.API) {
	router := chi.NewRouter()
	config := huma.DefaultConfig("Mind Forge Report API", "1.0.0")
	config.CreateHooks = nil
	config.Components.SecuritySchemes = map[string]*huma.SecurityScheme{
		"localBearer": {Type: "http", Scheme: "bearer"},
	}
	api := humachi.New(router, config)
	registerCatalog(api, options.Service, core.Renderer, "/v1/renderers", "list-renderers")
	registerCatalog(api, options.Service, core.Component, "/v1/components", "list-components")
	huma.Register(api, huma.Operation{OperationID: "report-health", Method: http.MethodGet, Path: "/health"},
		func(context.Context, *struct{}) (*statusOutput, error) { return status("ok"), nil })
	huma.Register(api, huma.Operation{OperationID: "report-ready", Method: http.MethodGet, Path: "/ready", Errors: []int{503}},
		func(context.Context, *struct{}) (*statusOutput, error) {
			if err := options.Service.Ready(options.Scope); err != nil {
				return nil, huma.Error503ServiceUnavailable("report project is unavailable", err)
			}
			return status("ready"), nil
		})
	huma.Register(api, huma.Operation{
		OperationID: "export-report", Method: http.MethodPost, Path: "/v1/reports/{slug}/exports",
		DefaultStatus: http.StatusOK, MaxBodyBytes: 8192,
		Errors:      []int{400, 401, 403, 404, 413, 422, 500, 503, 504},
		Security:    []map[string][]string{{"localBearer": {}}},
		Middlewares: huma.Middlewares{protectWrite(api, options.Address, options.Token)},
	}, func(ctx context.Context, input *exportInput) (*exportOutput, error) {
		request := core.Request{Slug: input.Slug}
		if input.Body != nil {
			request.Renderer, request.View, request.Theme = input.Body.Renderer, input.Body.View, input.Body.Theme
		}
		result, err := options.Service.Build(ctx, options.Scope, request)
		if err != nil {
			return nil, problem(err)
		}
		return &exportOutput{Body: result}, nil
	})
	if options.Scope.Reports != "" {
		huma.Register(api, huma.Operation{OperationID: "build-report-fragment", Method: http.MethodPost, Path: "/v1/reports/{slug}/fragments/{fragment}/exports", DefaultStatus: 200,
			Errors: []int{401, 403, 404, 409, 422, 500, 503, 504}, Security: []map[string][]string{{"localBearer": {}}}, Middlewares: huma.Middlewares{protectWrite(api, options.Address, options.Token)}}, func(ctx context.Context, input *fragmentInput) (*exportOutput, error) {
			result, err := options.Service.BuildFragment(ctx, options.Scope, input.Slug, input.Fragment)
			if err != nil {
				return nil, problem(err)
			}
			return &exportOutput{Body: result}, nil
		})
	}
	return router, api
}

func registerCatalog(api huma.API, service core.Service, kind core.Kind, path, operation string) {
	huma.Register(api, huma.Operation{OperationID: operation, Method: http.MethodGet, Path: path, Errors: []int{404, 422, 500}},
		func(_ context.Context, input *catalogInput) (*catalogOutput, error) {
			result, err := service.Catalog(kind, input.ID)
			if err != nil {
				return nil, problem(err)
			}
			return &catalogOutput{Body: result}, nil
		})
}

func status(value string) *statusOutput {
	response := &statusOutput{}
	response.Body.Status = value
	return response
}

func problem(err error) error {
	switch {
	case errors.Is(err, context.Canceled), errors.Is(err, context.DeadlineExceeded):
		return huma.Error504GatewayTimeout("report build canceled", err)
	case errors.Is(err, core.ErrConflict):
		return huma.Error409Conflict("asset publication conflict", err)
	case errors.Is(err, core.ErrNotFound):
		return huma.Error404NotFound("report resource not found", err)
	case errors.Is(err, core.ErrInvalid):
		return huma.Error422UnprocessableEntity("invalid report request", err)
	case errors.Is(err, core.ErrUnavailable):
		return huma.Error503ServiceUnavailable("report worker failed or is unavailable", err)
	default:
		return huma.Error500InternalServerError("report operation failed", err)
	}
}

func protectWrite(api huma.API, address, token string) func(huma.Context, func(huma.Context)) {
	host, port, _ := net.SplitHostPort(address)
	hosts := map[string]bool{strings.ToLower(address): true}
	if host == "127.0.0.1" || host == "::1" {
		hosts[net.JoinHostPort("localhost", port)] = true
	}
	return func(ctx huma.Context, next func(huma.Context)) {
		request, _ := humachi.Unwrap(ctx)
		origin := request.Header.Values("Origin")
		authority := strings.ToLower(request.Host)
		if address == "" || !hosts[authority] || len(origin) > 1 ||
			len(origin) == 1 && strings.ToLower(origin[0]) != "http://"+authority {
			_ = huma.WriteErr(api, ctx, http.StatusForbidden, "exports require a local Host and matching Origin")
			return
		}
		authorization := request.Header.Values("Authorization")
		if token == "" || len(authorization) != 1 ||
			subtle.ConstantTimeCompare([]byte(authorization[0]), []byte("Bearer "+token)) != 1 {
			ctx.SetHeader("WWW-Authenticate", "Bearer")
			_ = huma.WriteErr(api, ctx, http.StatusUnauthorized, "exports require the startup bearer token")
			return
		}
		ctx.SetHeader("Cache-Control", "no-store")
		next(ctx)
	}
}

func OpenAPI() ([]byte, error) {
	return Schema(true)
}

func Schema(project bool) ([]byte, error) {
	var scope core.Scope
	if project {
		scope.Reports = "reports"
	}
	_, api := NewAPI(APIOptions{Scope: scope})
	return api.OpenAPI().YAML()
}
