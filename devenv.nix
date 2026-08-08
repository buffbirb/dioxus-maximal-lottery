{
  # keep-sorted start
  config,
  lib,
  pkgs,
  # keep-sorted end
  ...
}:
let
  # keep-sorted start block=yes newline_separated=yes
  processConfigs = {
    # keep-sorted start block=yes
    web = {
      # keep-sorted start block=yes
      basePort = 8080;
      host = "127.0.0.1";
      # keep-sorted end
    };
    # keep-sorted end
  };

  serviceConfigs = {
    # keep-sorted start block=yes
    clickhouse = {
      # keep-sorted start block=yes
      host = "127.0.0.1";
      http = {
        # keep-sorted start block=yes
        basePort = 8123;
        # keep-sorted end
      };
      tcp = {
        # keep-sorted start block=yes
        basePort = 9000;
        # keep-sorted end
      };
      # keep-sorted end
    };
    otel = {
      # keep-sorted start block=yes
      grpc = {
        # keep-sorted start block=yes
        basePort = 4317;
        host = "127.0.0.1";
        # keep-sorted end
      };
      health = {
        # keep-sorted start block=yes
        basePort = 13133;
        host = "127.0.0.1";
        # keep-sorted end
      };
      http = {
        # keep-sorted start block=yes
        basePort = 4318;
        host = "127.0.0.1";
        # keep-sorted end
      };
      metrics = {
        # keep-sorted start block=yes
        basePort = 8888;
        host = "127.0.0.1";
        # keep-sorted end
      };
      # keep-sorted end
    };
    postgres = {
      # keep-sorted start block=yes
      basePort = 5432;
      host = "127.0.0.1";
      superuser = "postgres";
      superuser_password = "postgres";
      # keep-sorted end
    };
    # keep-sorted end
  };
  # keep-sorted end
in
{
  # keep-sorted start block=yes newline_separated=yes
  env = {
    # keep-sorted start block=yes
    DATABASE_URL = "postgres://${serviceConfigs.postgres.superuser}:${serviceConfigs.postgres.superuser_password}@${serviceConfigs.postgres.host}:${toString config.processes.postgres.ports.main.value}/postgres";
    # keep-sorted end
  };

  # https://devenv.sh/git-hooks/
  git-hooks = {
    # keep-sorted start block=yes
    hooks = {
      # keep-sorted start block=yes prefix_order=enable
      actionlint = {
        # keep-sorted start block=yes prefix_order=enable
        enable = true;
        # keep-sorted end
      };
      check-yaml = {
        # keep-sorted start block=yes prefix_order=enable
        enable = true;
        # keep-sorted end
      };
      deadnix = {
        # keep-sorted start block=yes prefix_order=enable
        enable = true;
        # keep-sorted end
      };
      dx-fmt = {
        # keep-sorted start block=yes prefix_order=enable
        enable = true;
        entry = "dx fmt --check";
        pass_filenames = false;
        # keep-sorted end
      };
      end-of-file-fixer = {
        # keep-sorted start block=yes prefix_order=enable
        enable = true;
        excludes = [
          # keep-sorted start
          "\\.lock$"
          "pnpm-lock\\.yaml$"
          # keep-sorted end
        ];
        # keep-sorted end
      };
      keep-sorted = {
        # keep-sorted start block=yes prefix_order=enable
        enable = true;
        after = [
          # keep-sorted start
          config.git-hooks.hooks.nixfmt.name
          # keep-sorted end
        ];
        # keep-sorted end
      };
      nixfmt = {
        # keep-sorted start block=yes prefix_order=enable
        enable = true;
        # keep-sorted end
      };
      rustfmt = {
        # keep-sorted start block=yes prefix_order=enable
        enable = true;
        # keep-sorted end
      };
      statix = {
        # keep-sorted start block=yes prefix_order=enable
        enable = true;
        # keep-sorted end
      };
      taplo = {
        # keep-sorted start block=yes prefix_order=enable
        enable = true;
        # keep-sorted end
      };
      trim-trailing-whitespace = {
        # keep-sorted start block=yes prefix_order=enable
        enable = true;
        excludes = [
          # keep-sorted start
          "\\.lock$"
          "pnpm-lock\\.yaml$"
          # keep-sorted end
        ];
        # keep-sorted end
      };
      # keep-sorted end
    };
    # keep-sorted end
  };

  languages = {
    # keep-sorted start block=yes newline_separated=yes
    rust = {
      # keep-sorted start block=yes prefix_order=enable
      enable = true;
      # https://github.com/cachix/devenv/blob/d59d872d80876d9eeb3e214d3b088bc4a14a9c4f/src/modules/languages/rust.nix#L311-L316
      channel = "stable";
      targets = [
        # keep-sorted start
        "wasm32-unknown-unknown"
        # keep-sorted end
      ];
      # keep-sorted end
    };
    # keep-sorted end
  };

  packages = with pkgs; [
    # keep-sorted start
    binaryen
    dioxus-cli
    sqlx-cli
    supabase-cli
    # keep-sorted end
  ];

  processes = {
    # keep-sorted start block=yes newline_separated=yes
    clickhouse-server = {
      # keep-sorted start block=yes
      ports = {
        # keep-sorted start block=yes
        http = {
          # keep-sorted start block=yes
          allocate = serviceConfigs.clickhouse.http.basePort;
          # keep-sorted end
        };
        main = {
          # keep-sorted start block=yes
          allocate = serviceConfigs.clickhouse.tcp.basePort;
          # keep-sorted end
        };
        # keep-sorted end
      };
      # keep-sorted end
    };

    opentelemetry-collector = {
      # keep-sorted start block=yes
      after = [
        # keep-sorted start
        "devenv:processes:clickhouse-server@ready"
        # keep-sorted end
      ];
      ports = {
        # keep-sorted start block=yes
        grpc = {
          # keep-sorted start block=yes
          allocate = serviceConfigs.otel.grpc.basePort;
          # keep-sorted end
        };
        health = {
          # keep-sorted start block=yes
          allocate = serviceConfigs.otel.health.basePort;
          # keep-sorted end
        };
        http = {
          # keep-sorted start block=yes
          allocate = serviceConfigs.otel.http.basePort;
          # keep-sorted end
        };
        metrics = {
          # keep-sorted start block=yes
          allocate = serviceConfigs.otel.metrics.basePort;
          # keep-sorted end
        };
        # keep-sorted end
      };
      ready = {
        # keep-sorted start block=yes
        http = {
          # keep-sorted start block=yes
          get = {
            # keep-sorted start block=yes
            port = lib.mkForce config.processes.opentelemetry-collector.ports.health.value;
            # keep-sorted end
          };
          # keep-sorted end
        };
        # keep-sorted end
      };
      # keep-sorted end
    };

    web = {
      # keep-sorted start block=yes
      after = [
        # keep-sorted start
        "db:migrate@succeeded"
        "devenv:processes:opentelemetry-collector@ready"
        "devenv:processes:postgres@ready"
        # keep-sorted end
      ];
      cwd = "packages/web";
      env = {
        # keep-sorted start
        BASIC_AUTH_ENABLED = "false";
        DATABASE_URL = "postgres://${serviceConfigs.postgres.superuser}:${serviceConfigs.postgres.superuser_password}@${serviceConfigs.postgres.host}:${toString config.processes.postgres.ports.main.value}/postgres";
        OTEL_EXPORTER_OTLP_ENDPOINT = "http://${serviceConfigs.otel.http.host}:${toString config.processes.opentelemetry-collector.ports.http.value}";
        # keep-sorted end
      };
      exec = "dx serve --web --addr ${processConfigs.web.host} --port ${toString config.processes.web.ports.http.value}";
      ports = {
        # keep-sorted start block=yes
        http = {
          # keep-sorted start block=yes
          allocate = processConfigs.web.basePort;
          # keep-sorted end
        };
        # keep-sorted end
      };
      ready = {
        # keep-sorted start block=yes
        http = {
          # keep-sorted start block=yes
          get = {
            # keep-sorted start block=yes
            path = "/";
            port = config.processes.web.ports.http.value;
            # keep-sorted end
          };
          # keep-sorted end
        };
        # keep-sorted end
      };
      # keep-sorted end
    };
    # keep-sorted end
  };

  services = {
    # keep-sorted start block=yes newline_separated=yes
    clickhouse = {
      # keep-sorted start block=yes prefix_order=enable
      enable = true;
      config = ''
        disable_internal_dns_cache: true
        listen_host: ${serviceConfigs.clickhouse.host}'';
      usersConfig = {
        # keep-sorted start block=yes
        profiles = {
          # keep-sorted start block=yes
          default = {
            # keep-sorted start block=yes
            compile_expressions = false;
            compile_sort_description = false;
            # keep-sorted end
          };
          # keep-sorted end
        };
        # keep-sorted end
      };
      # keep-sorted end
    };

    opentelemetry-collector = {
      # keep-sorted start prefix_order=enable
      enable = true;
      settings = {
        # keep-sorted start block=yes
        exporters = {
          # keep-sorted start block=yes
          clickhouse = {
            # keep-sorted start block=yes
            endpoint = "tcp://${serviceConfigs.clickhouse.host}:${toString config.processes.clickhouse-server.ports.main.value}";
            # keep-sorted end
          };
          # keep-sorted end
        };
        extensions = {
          # keep-sorted start block=yes
          health_check = {
            # keep-sorted start block=yes
            endpoint = lib.mkForce "${config.processes.opentelemetry-collector.ready.http.get.host}:${toString config.processes.opentelemetry-collector.ports.health.value}";
            # keep-sorted end
          };
          # keep-sorted end
        };
        processors = {
          # keep-sorted start block=yes
          batch = {
            # keep-sorted start block=yes
            # keep-sorted end
          };
          # keep-sorted end
        };
        receivers = {
          # keep-sorted start block=yes
          otlp = {
            # keep-sorted start block=yes
            protocols = {
              # keep-sorted start block=yes
              grpc = {
                # keep-sorted start block=yes
                endpoint = "${serviceConfigs.otel.grpc.host}:${toString config.processes.opentelemetry-collector.ports.grpc.value}";
                # keep-sorted end
              };
              http = {
                # keep-sorted start block=yes
                endpoint = "${serviceConfigs.otel.http.host}:${toString config.processes.opentelemetry-collector.ports.http.value}";
                # keep-sorted end
              };
              # keep-sorted end
            };
            # keep-sorted end
          };
          # keep-sorted end
        };
        service = {
          # keep-sorted start block=yes
          pipelines = {
            # keep-sorted start block=yes
            logs = {
              # keep-sorted start block=yes
              exporters = [
                # keep-sorted start
                "clickhouse"
                # keep-sorted end
              ];
              processors = [
                # keep-sorted start
                "batch"
                # keep-sorted end
              ];
              receivers = [
                # keep-sorted start
                "otlp"
                # keep-sorted end
              ];
              # keep-sorted end
            };
            metrics = {
              # keep-sorted start block=yes
              exporters = [
                # keep-sorted start
                "clickhouse"
                # keep-sorted end
              ];
              processors = [
                # keep-sorted start
                "batch"
                # keep-sorted end
              ];
              receivers = [
                # keep-sorted start
                "otlp"
                # keep-sorted end
              ];
              # keep-sorted end
            };
            traces = {
              # keep-sorted start block=yes
              exporters = [
                # keep-sorted start
                "clickhouse"
                # keep-sorted end
              ];
              processors = [
                # keep-sorted start
                "batch"
                # keep-sorted end
              ];
              receivers = [
                # keep-sorted start
                "otlp"
                # keep-sorted end
              ];
              # keep-sorted end
            };
            # keep-sorted end
          };
          telemetry = {
            # keep-sorted start block=yes
            metrics = {
              # keep-sorted start block=yes
              readers = [
                # keep-sorted start block=yes
                {
                  # keep-sorted start block=yes
                  pull = {
                    # keep-sorted start block=yes
                    exporter = {
                      # keep-sorted start block=yes
                      prometheus = {
                        # keep-sorted start block=yes
                        host = serviceConfigs.otel.metrics.host;
                        port = config.processes.opentelemetry-collector.ports.metrics.value;
                        # keep-sorted end
                      };
                      # keep-sorted end
                    };
                    # keep-sorted end
                  };
                  # keep-sorted end
                }
                # keep-sorted end
              ];
              # keep-sorted end
            };
            # keep-sorted end
          };
          # keep-sorted end
        };
        # keep-sorted end
      };
      # keep-sorted end
    };

    postgres = {
      # keep-sorted start block=yes prefix_order=enable
      enable = true;
      initialScript = "CREATE ROLE ${serviceConfigs.postgres.superuser} WITH LOGIN SUPERUSER PASSWORD '${serviceConfigs.postgres.superuser_password}'";
      listen_addresses = serviceConfigs.postgres.host;
      package = pkgs.postgresql_17;
      port = serviceConfigs.postgres.basePort;
      # keep-sorted end
    };
    # keep-sorted end
  };

  tasks = {
    # keep-sorted start block=yes
    "db:migrate" = {
      # keep-sorted start block=yes
      after = [
        # keep-sorted start
        "devenv:processes:postgres@ready"
        # keep-sorted end
      ];
      description = "Apply pending Supabase migrations to the local Postgres database";
      exec = "sqlx migrate run --source supabase/migrations";
      showOutput = true;
      # keep-sorted end
    };
    "db:reset" = {
      # keep-sorted start block=yes
      after = [
        # keep-sorted start
        "devenv:processes:postgres@ready"
        # keep-sorted end
      ];
      description = "Drop and recreate the public schema, resetting the database to a blank slate";
      exec = "psql \"postgres://${serviceConfigs.postgres.superuser}:${serviceConfigs.postgres.superuser_password}@${serviceConfigs.postgres.host}:${toString config.processes.postgres.ports.main.value}/postgres\" -v ON_ERROR_STOP=1 -c 'DROP SCHEMA public CASCADE; CREATE SCHEMA public;'";
      showOutput = true;
      # keep-sorted end
    };
    # keep-sorted end
  };
  # keep-sorted end
}
