var builder = DistributedApplication.CreateBuilder(args);

const int webPort = 8080;
// A Docker sandbox forwards the port only to a server listening on every interface.
var webHost = builder.Configuration["WEB_HOST"] ?? "127.0.0.1";
// The dev GitHub App's sign-in callback is registered on this origin.
var webOrigin = $"http://127.0.0.1:{webPort}";

// The credentials and port .cargo/config.toml's DATABASE_URL uses.
var postgres = builder
    .AddPostgres("postgres", password: builder.AddParameter("postgres-password", "postgres"), port: 5432)
    // Supabase runs Postgres 17; Aspire's default image is a newer major.
    .WithImageTag("17-alpine")
    .WithDataVolume()
    .WithPgWeb();
var database = postgres.AddDatabase("lottery");

var migrate = builder
    .AddExecutable("migrate", "sqlx", "..", "migrate", "run", "--source", "supabase/migrations")
    .WithEnvironment("DATABASE_URL", database.Resource.UriExpression)
    .WaitFor(database);

// --force ends the server's open connections first.
builder
    .AddExecutable("reset-database", "sqlx", "..", "database", "reset", "--force", "--source", "supabase/migrations", "-y")
    .WithEnvironment("DATABASE_URL", database.Resource.UriExpression)
    .WaitFor(database)
    .WithExplicitStart();

builder
    .AddExecutable(
        "web", "dx", "..",
        "serve",
        "--addr", webHost,
        "--interactive", "false",
        "--open", "false",
        "--package", "web",
        "--platform", "web",
        "--port", webPort.ToString())
    .WithHttpEndpoint(port: webPort, name: "http", isProxied: false)
    .WithUrlForEndpoint("http", url => url.Url = webOrigin)
    .WithEnvironment("BASIC_AUTH_ENABLED", "false")
    .WithEnvironment("DATABASE_URL", database.Resource.UriExpression)
    .WithEnvironment("OAUTH_GITHUB_CLIENT_ID", builder.AddParameter("oauth-github-client-id"))
    .WithEnvironment("OAUTH_GITHUB_CLIENT_SECRET", builder.AddParameter("oauth-github-client-secret", secret: true))
    // Otherwise the server derives its origin from the address it listens on.
    .WithEnvironment("PUBLIC_BASE_URL", webOrigin)
    // The server exports OTLP over HTTP, not gRPC.
    .WithOtlpExporter(OtlpProtocol.HttpProtobuf)
    .WithHttpHealthCheck("/health")
    .WaitForCompletion(migrate);

builder.Build().Run();
