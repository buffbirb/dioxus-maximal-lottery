using Microsoft.Extensions.Configuration;

var builder = DistributedApplication.CreateBuilder(args);

var postgres = builder
    .AddPostgres("postgres")
    // Supabase runs Postgres 17; Aspire's default image is a newer major.
    .WithImageTag("17-alpine")
    .WithDataVolume()
    .WithPgWeb();
var database = postgres.AddDatabase("lottery");

var migrate = builder
    .AddExecutable("migrate", "sqlx", "..", "migrate", "run", "--source", "supabase/migrations")
    .WithEnvironment("DATABASE_URL", database.Resource.UriExpression)
    .WaitFor(database);

builder
    .AddExecutable("prepare-queries", "cargo", "..", "sqlx", "prepare", "--workspace", "--", "--all-features", "--all-targets")
    .WithEnvironment("DATABASE_URL", database.Resource.UriExpression)
    .WaitForCompletion(migrate)
    .WithExplicitStart();

// --force ends the server's open connections first.
builder
    .AddExecutable("reset-database", "sqlx", "..", "database", "reset", "--force", "--source", "supabase/migrations", "-y")
    .WithEnvironment("DATABASE_URL", database.Resource.UriExpression)
    .WaitFor(database)
    .WithExplicitStart();

// WEB_HOST and WEB_PORT are for a Docker sandbox, which forwards only published
// ports, and only to servers listening on every interface.
var web = builder
    .AddExecutable("web", "dx", "..")
    .WithHttpEndpoint(port: builder.Configuration.GetValue<int?>("WEB_PORT"), name: "http", isProxied: false);
var http = web.GetEndpoint("http");
// The local OAuth App's loopback callback is on 127.0.0.1, so the browser has to be too.
var origin = ReferenceExpression.Create($"http://127.0.0.1:{http.Property(EndpointProperty.Port)}");
web
    .WithArgs(
        "serve",
        "--addr", builder.Configuration["WEB_HOST"] ?? "127.0.0.1",
        "--interactive", "false",
        "--open", "false",
        "--package", "web",
        "--platform", "web",
        "--port", http.Property(EndpointProperty.TargetPort))
    .WithUrlForEndpoint("http", url => url.Url = new UriBuilder(url.Url) { Host = "127.0.0.1" }.Uri.ToString())
    .WithEnvironment("BASIC_AUTH_ENABLED", "false")
    .WithEnvironment("DATABASE_URL", database.Resource.UriExpression)
    .WithEnvironment("OAUTH_GITHUB_CLIENT_ID", builder.AddParameter("oauth-github-client-id"))
    .WithEnvironment("OAUTH_GITHUB_CLIENT_SECRET", builder.AddParameter("oauth-github-client-secret", secret: true))
    // Otherwise the server derives its origin from the address it listens on.
    .WithEnvironment("PUBLIC_BASE_URL", origin)
    // The server exports OTLP over HTTP, not gRPC.
    .WithOtlpExporter(OtlpProtocol.HttpProtobuf)
    .WithHttpHealthCheck("/health")
    .WaitForCompletion(migrate);

builder.Build().Run();
