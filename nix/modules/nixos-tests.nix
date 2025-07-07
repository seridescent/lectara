{ inputs, self, ... }: {
  perSystem = { config, pkgs, lib, ... }:
    let
      mkTest = name: testConfig: pkgs.testers.runNixOSTest {
        inherit name;
        nodes = {
          server = {
            imports = [
              self.nixosModules.default
              testConfig.serverConfig
            ];
          };

          client = {
            imports = [
              testConfig.clientConfig
            ];
          };
        };

        testScript = testConfig.script;
      };

      allTests = {
        service-smoke = mkTest "lectara-service-smoke" {
          serverConfig = { pkgs, ... }: {
            services.lectara = {
              enable = true;
              port = 3000;
            };

            # Open firewall for the test
            networking.firewall.enable = false;

            # Add sqlite for database queries in tests
            environment.systemPackages = [ pkgs.sqlite ];
          };

          clientConfig = { pkgs, ... }: {
            # Minimal client with curl and jq for testing
            environment.systemPackages = with pkgs;
              [ curl jq ];
          };

          script = ''
            import json

            # Start all machines
            start_all()

            # Wait for the lectara service to be ready
            server.wait_for_unit("lectara.service")
          
            # Give the service a moment to fully initialize
            server.sleep(2)
          
            # Check that the service started successfully
            server.succeed("systemctl is-active lectara.service")
          
            # Check service logs for startup confirmation
            # Adjust this based on your actual log output
            server.succeed("journalctl -u lectara.service | grep 'Server running on http://'")
          
            # Test the health endpoint first
            #client.wait_until_succeeds(
            #    "curl -f http://server:3000/health",
            #    timeout=30
            #)
          
            # Send a POST request to create content
            response = client.succeed("""
                curl -s -X POST http://server:3000/api/v1/content \
                  -H 'Content-Type: application/json' \
                  -d '{"url": "https://example.com/article", "title": "Test Article", "author": "Test Author"}' \
                  -w '\n%{http_code}'
            """)
          
            # Extract status code (last line)
            lines = response.strip().split('\n')
            status_code = lines[-1]
            json_response = '\n'.join(lines[:-1])
          
            # Check for successful response (200 or 201)
            assert status_code in ["200", "201"], f"Expected 200/201, got {status_code}"
          
            # Parse the response to get the created item (if your API returns it)
            if json_response:
                created_item = json.loads(json_response)
                print(f"Created item: {created_item}")
          
            # Give the database write a moment to complete
            server.sleep(1)
          
            # Query the database to verify the content was saved
            db_result = server.succeed("""
                sqlite3 /var/lib/lectara/lectara.db \
                  "SELECT url, title, author FROM content_items WHERE url='https://example.com/article';"
            """)
          
            # Verify the database contains our test data
            assert "https://example.com/article" in db_result, "URL not found in database"
            assert "Test Article" in db_result, "Title not found in database"
            assert "Test Author" in db_result, "Author not found in database"
          
            # Test another POST to ensure multiple items work
            client.succeed("""
                curl -s -X POST http://server:3000/api/v1/content \
                  -H 'Content-Type: application/json' \
                  -d '{"url": "https://example.com/another", "title": "Another Article"}' \
                  -o /dev/null \
                  -w '%{http_code}' | grep -E '^20[01]$'
            """)
          
            # Verify we now have 2 items
            count = server.succeed("""
                sqlite3 /var/lib/lectara/lectara.db \
                  "SELECT COUNT(*) FROM content_items;"
            """).strip()
          
            assert count == "2", f"Expected 2 items in database, got {count}"
          
            # Test with minimal data (only URL)
            client.succeed("""
                curl -s -X POST http://server:3000/api/v1/content \
                  -H 'Content-Type: application/json' \
                  -d '{"url": "https://example.com/minimal"}' \
                  -o /dev/null \
                  -w '%{http_code}' | grep -E '^20[01]$'
            """)
          
            # Final check - ensure all 3 items are in the database
            all_urls = server.succeed("""
                sqlite3 /var/lib/lectara/lectara.db \
                  "SELECT url FROM content_items ORDER BY created_at;"
            """)
          
            assert "https://example.com/article" in all_urls
            assert "https://example.com/another" in all_urls
            assert "https://example.com/minimal" in all_urls
          
            print("All tests passed! 🎉")
          '';
        };
      };
    in
    {
      packages = (lib.mapAttrs'
        (name: value:
          lib.nameValuePair "test-${name}" value
        )
        allTests)
      // {
        test-all = pkgs.symlinkJoin {
          name = "lectara-all";
          paths = lib.attrValues allTests;
        };
      };
    };
}
