{ withSystem, ... }: {
  flake.nixosModules.default = { lib, config, pkgs, ... }:
    let
      cfg = config.services.lectara;
    in
    {
      options = {
        services.lectara = {
          enable = lib.mkEnableOption "Lectara content collection service";

          package = lib.mkOption {
            description = "The lectara-service package to use";
            default = withSystem pkgs.stdenv.hostPlatform.system ({ config, ... }:
              config.packages.lectara-service
            );
            type = lib.types.package;
          };

          port = lib.mkOption {
            # TODO: come back here when we make ports configurable
            description = "Port for the lectara service to listen on. WARNING: currently doesn't do anything";
            default = 3000;
            type = lib.types.port;
          };

          openFirewall = lib.mkOption {
            description = "Whether to open the firewall for the service's port";
            default = false;
            type = lib.types.bool;
          };

          databasePath = lib.mkOption {
            description = "Path to the SQLite database file";
            default = "/var/lib/lectara/lectara.db";
            type = lib.types.path;
          };
        };
      };

      config = lib.mkIf cfg.enable {
        systemd.services.lectara = {
          description = "Lectara content collection service";
          wantedBy = [ "multi-user.target" ];

          environment = {
            DATABASE_URL = "sqlite://${cfg.databasePath}";
            RUST_LOG = "info";
          };

          serviceConfig = {
            ExecStart = "${cfg.package}/bin/lectara-service";
            Restart = "on-failure";
            RestartSec = 5;

            # Security hardening
            DynamicUser = true;
            StateDirectory = "lectara";
            PrivateTmp = true;
            ProtectSystem = "strict";
            ProtectHome = true;
            NoNewPrivileges = true;

            # Ensure the database directory exists
            RuntimeDirectory = "lectara";
            RuntimeDirectoryMode = "0700";
          };
        };

        # Open the firewall if needed
        networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ cfg.port ];
      };
    };
}
