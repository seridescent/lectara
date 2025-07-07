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
            description = "Open the firewall for the service's port";
            default = false;
            type = lib.types.bool;
          };

          user = lib.mkOption {
            description = "User account under which the service runs";
            default = "lectara";
            type = lib.types.str;
          };

          group = lib.mkOption {
            description = "Group under which the service runs";
            default = "lectara";
            type = lib.types.str;
          };

          baseDir = lib.mkOption {
            description = "Base directory for the service (used for WorkingDir and database parent)";
            default = "/var/lib/lectara";
            type = lib.types.path;
          };
        };
      };

      config = lib.mkIf cfg.enable {
        # Create user and group
        users.users = lib.mkIf (cfg.user == "lectara") {
          lectara = {
            inherit (cfg) group;
            isSystemUser = true;
          };
        };

        users.groups = lib.mkIf (cfg.group == "lectara") {
          lectara = { };
        };

        # Ensure directories exist with proper permissions
        systemd.tmpfiles.settings.lectaraDirs = {
          "${cfg.baseDir}"."d" = {
            mode = "750";
            inherit (cfg) user group;
          };
          "${cfg.baseDir}/data"."d" = {
            mode = "750";
            inherit (cfg) user group;
          };
        };

        systemd.services.lectara = {
          description = "Lectara content collection service";
          wantedBy = [ "multi-user.target" ];

          environment = {
            DATABASE_URL = "sqlite://${cfg.baseDir}/data/lectara.db";
            RUST_LOG = "info";
          };

          serviceConfig = {
            ExecStart = "${cfg.package}/bin/lectara-service";
            Restart = "on-failure";
            RestartSec = 5;

            # User and directory configuration
            User = cfg.user;
            Group = cfg.group;
            WorkingDirectory = cfg.baseDir;

            # Security hardening
            PrivateTmp = true;
            NoNewPrivileges = true;
          };
        };

        networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ cfg.port ];
      };
    };
}
