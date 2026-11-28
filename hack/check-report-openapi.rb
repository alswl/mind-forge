# Compare the actual CLI exports after validating the one project-only operation.
require "yaml"

abort "usage: ruby #{$PROGRAM_NAME} PROJECT.yaml STANDALONE.yaml" unless ARGV.size == 2
project, standalone = ARGV.map { |path| YAML.safe_load(File.read(path)) }
fragment_path = "/v1/reports/{slug}/fragments/{fragment}/exports"
shared_export = "/v1/reports/{slug}/exports"
paths = project.fetch("paths")
abort "standalone advertises a project asset route" if standalone.fetch("paths").key?(fragment_path)
fragment = paths.delete(fragment_path)
abort "project fragment must expose POST" unless fragment && fragment.keys == ["post"]
post = fragment.fetch("post")
abort "wrong fragment operation ID" unless post.fetch("operationId") == "build-report-fragment"
abort "wrong fragment authorization" unless post.fetch("security") == [{"localBearer" => []}]
abort "fragment export takes no request body" if post.key?("requestBody")

# Both fragment identifiers follow the report slug contract, including length and pattern.
export = paths.fetch(shared_export).fetch("post")
slug = export.fetch("parameters").find { |p| p["name"] == "slug" && p["in"] == "path" && p["required"] }
abort "missing shared report slug parameter" unless slug
parameters = post.fetch("parameters").sort_by { |p| p.fetch("name") }
abort "wrong fragment path parameters" unless parameters == [slug.merge("name" => "fragment"), slug]

responses = post.fetch("responses")
expected_codes = %w[200 401 403 404 409 422 500 503 504]
abort "wrong fragment response codes" unless responses.keys.sort == expected_codes
schemas = project.fetch("components").fetch("schemas")
responses.each do |code, response|
  media, model = code == "200" ? ["application/json", "Result"] : ["application/problem+json", "ErrorModel"]
  schema = response.fetch("content").fetch(media).fetch("schema")
  abort "wrong fragment response schema for #{code}" unless schema == {"$ref" => "#/components/schemas/#{model}"} && schemas.key?(model)
end

# Fragment parameters are inline. Result and ErrorModel are shared. There are
# no project-only named schemas to strip before comparing the complete documents.
%w[/health /ready /v1/renderers /v1/components].each { |path| paths.fetch(path).fetch("get") }
abort "shared OpenAPI contract differs between CLIs" unless project == standalone
puts "Verified #{paths.size} shared paths and #{schemas.size} shared schemas, plus the project-only fragment export"
