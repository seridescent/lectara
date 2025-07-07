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
                sqlite3 /var/lib/lectara/data/lectara.db \
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
                sqlite3 /var/lib/lectara/data/lectara.db \
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
                sqlite3 /var/lib/lectara/data/lectara.db \
                  "SELECT url FROM content_items ORDER BY created_at;"
            """)
          
            assert "https://example.com/article" in all_urls
            assert "https://example.com/another" in all_urls
            assert "https://example.com/minimal" in all_urls
          
            print("All tests passed! 🎉")
          '';
        };

        # Add this to your allTests in nixos-tests.nix
        service-reboot = mkTest "lectara-service-reboot" {
          serverConfig = { pkgs, ... }: {
            services.lectara = {
              enable = true;
              port = 3000;
            };

            networking.firewall.enable = false;
            environment.systemPackages = [ pkgs.sqlite ];
          };

          clientConfig = { pkgs, ... }: {
            environment.systemPackages = with pkgs; [ curl jq ];
          };

          script = ''
            import json
    
            start_all()
    
            # Initial service startup
            server.wait_for_unit("lectara.service")
            server.sleep(2)
    
            # Create test data with known content
            print("Creating test data...")
            test_data = []
            for i in range(10):
                item = {
                    "url": f"https://example.com/reboot-test-{i}",
                    "title": f"Reboot Test Article {i}",
                    "author": f"Author {i}",
                    "body": f"This is test content for article {i}. " * 10
                }
                test_data.append(item)
        
                client.succeed(f"""
                    curl -s -X POST http://server:3000/api/v1/content \
                      -H 'Content-Type: application/json' \
                      -d '{json.dumps(item)}' \
                      -o /dev/null
                """)
    
            # Get a checksum of the database content
            print("Calculating database checksum...")
            pre_reboot_data = server.succeed("""
                sqlite3 /var/lib/lectara/data/lectara.db \
                  "SELECT url, title, author, body FROM content_items ORDER BY url;" | sha256sum
            """).strip()
    
            # Get the count
            pre_reboot_count = server.succeed("""
                sqlite3 /var/lib/lectara/data/lectara.db \
                  "SELECT COUNT(*) FROM content_items;"
            """).strip()
    
            print(f"Pre-reboot: {pre_reboot_count} items, checksum: {pre_reboot_data[:16]}...")
    
            # Simulate an unclean shutdown (kill -9)
            print("Simulating unclean shutdown...")
            server.succeed("systemctl kill -s KILL lectara.service || true")
            server.sleep(1)
    
            # Reboot the server
            print("Rebooting server...")
            server.shutdown()
            server.start()
            server.wait_for_unit("multi-user.target")
    
            # Wait for service to come up after reboot
            server.wait_for_unit("lectara.service")
            server.sleep(2)
    
            # Verify service is running
            server.succeed("systemctl is-active lectara.service")
    
            # Check data integrity
            # admittedly unlikely to be necessary, thanks SQLite
            print("Verifying data integrity after reboot...")
            post_reboot_data = server.succeed("""
                sqlite3 /var/lib/lectara/data/lectara.db \
                  "SELECT url, title, author, body FROM content_items ORDER BY url;" | sha256sum
            """).strip()
    
            post_reboot_count = server.succeed("""
                sqlite3 /var/lib/lectara/data/lectara.db \
                  "SELECT COUNT(*) FROM content_items;"
            """).strip()
    
            print(f"Post-reboot: {post_reboot_count} items, checksum: {post_reboot_data[:16]}...")
    
            # Verify counts match
            assert pre_reboot_count == post_reboot_count, \
                f"Data loss! Pre: {pre_reboot_count}, Post: {post_reboot_count}"
    
            # Verify checksums match (all data intact)
            assert pre_reboot_data == post_reboot_data, \
                "Data corruption detected! Checksums don't match"
    
            # Test that service is fully functional
            print("Testing service functionality after reboot...")
    
            # Test creating new content
            new_response = client.succeed("""
                curl -s -X POST http://server:3000/api/v1/content \
                  -H 'Content-Type: application/json' \
                  -d '{"url": "https://example.com/after-reboot", "title": "Created After Reboot"}' \
                  -w '\n%{http_code}'
            """)
    
            status_code = new_response.strip().split('\n')[-1]
            assert status_code in ["200", "201"], f"Service not functioning after reboot: {status_code}"
    
            # Test idempotency with existing data
            existing_response = client.succeed(f"""
                curl -s -X POST http://server:3000/api/v1/content \
                  -H 'Content-Type: application/json' \
                  -d '{json.dumps(test_data[4])}' \
                  -w '\n%{{http_code}}'
            """)
    
            status_code2 = existing_response.strip().split('\n')[-1]
            assert status_code2 == "200", f"Idempotency broken after reboot: {status_code2}"
    
            # Verify the new count is correct
            final_count = server.succeed("""
                sqlite3 /var/lib/lectara/data/lectara.db \
                  "SELECT COUNT(*) FROM content_items;"
            """).strip()
    
            assert final_count == "11", f"Expected 11 items, got {final_count}"
    
            print("Reboot resilience test passed! 💪")
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
