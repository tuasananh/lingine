import os
import subprocess
import sys
import threading
from http.server import SimpleHTTPRequestHandler, HTTPServer

class HealthCheckHandler(SimpleHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-type", "text/plain")
        self.end_headers()
        self.wfile.write(b"OK")
        
    def log_message(self, format, *args):
        # Suppress logging of successful requests to keep logs clean
        pass

def start_health_check_server():
    port = int(os.environ.get("PORT", 8080))
    server = HTTPServer(("0.0.0.0", port), HealthCheckHandler)
    print(f"Starting health check server on port {port}...")
    server.serve_forever()

def main():
    # Start health check server in a daemon thread if PORT is defined (which is typical on Render)
    if "PORT" in os.environ:
        t = threading.Thread(target=start_health_check_server, daemon=True)
        t.start()
    else:
        print("PORT environment variable not found. Health check server will not start.")
        
    # Start lichess-bot.py
    cmd = [sys.executable, "lichess-bot.py"]
    # Pass any arguments received by start.py to lichess-bot.py
    if len(sys.argv) > 1:
        cmd.extend(sys.argv[1:])
        
    print(f"Starting lichess-bot with command: {' '.join(cmd)}")
    try:
        sys.exit(subprocess.call(cmd))
    except KeyboardInterrupt:
        print("\nExiting bot wrapper...")
        sys.exit(0)

if __name__ == "__main__":
    main()
