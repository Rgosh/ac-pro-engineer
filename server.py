import socket
import threading
import sys


HOST = '0.0.0.0'
PORT = 6000       

def handle_client(conn, addr):
    print(f"[NEW CONNECTION] {addr} connected.")
    try:
        conn.sendall(b"Welcome to AVN PRO Server!\n")
        while True:
            data = conn.recv(1024)
            if not data:
                break
            print(f"[{addr}] {data.decode('utf-8', errors='ignore').strip()}")
            conn.sendall(b"OK: " + data)
    except Exception as e:
        print(f"Error: {e}")
    finally:
        conn.close()

def start_server():
    print(f"[STARTING] Server is starting on port {PORT}...")
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        server.bind((HOST, PORT))
        server.listen()
        print(f"[LISTENING] Server is listening on {HOST}:{PORT}")
        while True:
            conn, addr = server.accept()
            t = threading.Thread(target=handle_client, args=(conn, addr))
            t.start()
    except Exception as e:
        print(f"CRITICAL ERROR: {e}")
    finally:
        server.close()

if __name__ == "__main__":
    sys.stdout.reconfigure(line_buffering=True)
    start_server()